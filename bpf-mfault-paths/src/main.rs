use anyhow::Result;
use blazesym::symbolize;
use clap::Parser;
use libbpf_rs::skel::OpenSkel;
use libbpf_rs::skel::Skel;
use libbpf_rs::skel::SkelBuilder;
use libbpf_rs::PerfBufferBuilder;
use std::cell::RefCell;
use std::collections::hash_map;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;
use std::mem::MaybeUninit;
use std::time::Duration;
use time::macros::format_description;
use time::OffsetDateTime;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

mod mmfault {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bpf/mmfault_paths.skel.rs"
    ));
}

mod consts;

#[derive(Copy, Clone, Debug)]
enum Event {
    InodePath(InodePathEvent),
    PageFault(PageFaultEvent),
}

#[derive(FromBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C)]
struct InodePathEvent {
    inode: u64,
    component: [u8; consts::NAME_MAX],
}

#[derive(FromBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C)]
struct BpfStackBuildId {
    status: i32,
    build_id: [u8; 20],
    offset_or_ip: u64,
}

#[derive(FromBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C)]
struct PageFaultEvent {
    pid: u32,
    tid: u32,
    vm_flags: u32,
    fault_flags: u32,
    inode: u64,
    off: u64,

    // stack: [BpfStackBuildId; 16],
    stack_size: u32,
    stack: [u64; 16],
}

impl Event {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        let (event_type, data) = u64::read_from_prefix(data).ok()?;
        let mut data_aligned = [0u64; 100];
        let data_aligned = data_aligned.as_mut_bytes().get_mut(..data.len())?;
        data_aligned.copy_from_slice(data);
        Some(match event_type {
            0 => Event::InodePath(*InodePathEvent::ref_from_prefix(data_aligned).ok()?.0),
            1 => Event::PageFault(*PageFaultEvent::ref_from_prefix(data_aligned).ok()?.0),
            _ => return None,
        })
    }
}

/// Trace page faults and record file offsets.
#[derive(Parser, Debug)]
struct Cmd {
    /// Trace only given PID
    #[arg(short, long)]
    pid: u32,

    #[arg(short, long, default_value = "10")]
    min_faults: u64,

    /// Verbose libbpf output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Default)]
struct PathInProgress {
    inode: u64,
    components: Vec<Vec<u8>>,
}

struct State {
    paths: HashMap<u64, String>,
    paths_in_progress: HashMap<i32, PathInProgress>,

    last_acessed_pages: HashMap<u64, Vec<u64>>,

    symbolizer: symbolize::Symbolizer,

    stats_begin: std::time::Instant,
    stats: HashMap<
        (u32, u32, Vec<u64>),               // (vm_flags, fault_flags, stack)
        (BTreeMap<u64, u64>, HashSet<u64>), // (Histogram: dist -> count, set of inodes)
    >,
}

impl State {
    fn new() -> Self {
        Self {
            paths: HashMap::new(),
            paths_in_progress: HashMap::new(),
            last_acessed_pages: HashMap::new(),
            symbolizer: symbolize::Symbolizer::new(),
            stats_begin: std::time::Instant::now(),
            stats: HashMap::new(),
        }
    }

    fn path(&self, inode: u64) -> &str {
        self.paths.get(&inode).map_or("<unknown>", |p| p.as_str())
    }

    fn handle_event(&mut self, cpu: i32, data: &[u8]) {
        let Some(event) = Event::from_bytes(data) else {
            return;
        };

        if let Some(pip) = self.paths_in_progress.get(&cpu) {
            if !matches!(&event, Event::InodePath(evt) if evt.inode == pip.inode) {
                let pip = self.paths_in_progress.remove(&cpu).unwrap();
                self.paths.insert(
                    pip.inode,
                    String::from_utf8_lossy(&rev_join(pip.components)).into_owned(),
                );
            }
        }

        match &event {
            Event::InodePath(evt) => {
                if self.paths.contains_key(&evt.inode) {
                    // Path already known, skip
                    return;
                }
                let component = nul_termiate(&evt.component);
                self.paths_in_progress
                    .entry(cpu)
                    .and_modify(|pip| pip.components.push(component.to_vec()))
                    .or_insert_with(|| PathInProgress {
                        inode: evt.inode,
                        components: vec![component.to_vec()],
                    });
            }
            Event::PageFault(evt) => {
                let now = OffsetDateTime::now_local()
                    .ok()
                    .and_then(|t| {
                        let fmt = format_description!("[hour]:[minute]:[second]");
                        t.format(&fmt).ok()
                    })
                    .unwrap_or_else(|| "00:00:00".to_string());
                let _ = now;

                // println!(
                //     "{now:>8} {pid:>6}/{tid:<6} inode:{inode} off:{off} flags:0x{flags:x} path:{path}",
                //     pid = evt.pid,
                //     tid = evt.tid,
                //     inode = evt.inode,
                //     off = evt.off,
                //     flags = evt.flags,
                //     path = self.path(evt.inode),
                // );

                match self.last_acessed_pages.entry(evt.inode) {
                    hash_map::Entry::Vacant(e) => {
                        e.insert(vec![evt.off]);
                    }
                    hash_map::Entry::Occupied(mut e) => {
                        let lru = e.get_mut();

                        // Calculate distance to the nearest different offset (if any)
                        let off = evt.off;
                        let min_dist = lru
                            .iter()
                            .filter(|&&x| x != off)
                            .map(|&x| if x > off { x - off } else { off - x })
                            .min();

                        // Move existing entry (if present) to the end, or append it if new
                        if let Some(pos) = lru.iter().position(|&x| x == evt.off) {
                            lru.remove(pos);
                        }
                        lru.push(evt.off);

                        // Enforce max LRU size
                        if lru.len() > 32 {
                            lru.remove(0);
                        }

                        // Update histogram
                        if let Some(dist) = min_dist {
                            let stack = evt.stack[0..evt.stack_size as usize].to_vec();
                            let entry = self
                                .stats
                                .entry((evt.vm_flags, evt.fault_flags, stack))
                                .or_default();
                            entry.1.insert(evt.inode);
                            *entry.0.entry(dist.min(199999)).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cmd = Cmd::parse();

    // Build skeleton
    let mut skel_builder = mmfault::MmfaultPathsSkelBuilder::default();
    if cmd.verbose {
        skel_builder.obj_builder.debug(true);
    }

    let mut open_obj = MaybeUninit::uninit();
    let mut open_skel = skel_builder.open(&mut open_obj)?;

    // Configure rodata
    if let Some(ro) = open_skel.maps.rodata_data.as_deref_mut() {
        ro.target_pid = cmd.pid as i32;
    }

    let mut skel = open_skel.load()?;
    skel.attach()?;

    println!("Tracing handle_mm_fault (PID filter: {})", cmd.pid);

    let state = RefCell::new(State::new());

    let perf = PerfBufferBuilder::new(&skel.maps.events)
        .sample_cb(|cpu, data| {
            let mut st = state.borrow_mut();
            st.handle_event(cpu, data);
        })
        .lost_cb(|cpu, count| {
            eprintln!("Lost {count} events on CPU {cpu}");
        })
        .pages(4096) // Huge buffer so we don't lose events
        .build()?;

    loop {
        perf.poll(Duration::from_millis(100))?;

        let mut st = state.borrow_mut();
        if st.stats_begin.elapsed().as_secs() >= 1 {
            let src =
                symbolize::source::Source::from(symbolize::source::Process::new(cmd.pid.into()));

            println!("\nStatistics:");

            for ((vm_flags, fault_flags, stack), (hist, inodes)) in &st.stats {
                // Check the amount of faults within a distance of a single page,
                // aka sequential access.
                const PAGE_SIZE: u64 = 4096;
                if hist.get(&PAGE_SIZE).is_none_or(|&c| c < cmd.min_faults) {
                    continue;
                }

                println!("● flags = \x1b[1m{:x}\x1b[m / {:x}", vm_flags, fault_flags);

                for inode in inodes {
                    println!("  \x1b[33m{}\x1b[m", st.path(*inode));
                }

                for (dist, count) in hist {
                    println!("  Distance: \x1b[32m{dist:5}\x1b[m Count: \x1b[31m{count}\x1b[m");
                }
                match st
                    .symbolizer
                    .symbolize(&src, symbolize::Input::AbsAddr(stack))
                {
                    Ok(symbols) => {
                        for (no, symbol) in symbols.iter().enumerate() {
                            match symbol {
                                symbolize::Symbolized::Sym(sym) => {
                                    let mut line = format!(
                                        "{:2}. \x1b[34m{name}\x1b[m",
                                        no + 1,
                                        name = sym.name,
                                    );
                                    if let Some(info) = &sym.code_info {
                                        write!(line, " @ \x1b[35m").unwrap();
                                        if let Some(dir) = &info.dir {
                                            write!(line, "{}/", dir.display()).unwrap();
                                        }
                                        write!(line, "{}", info.file.display()).unwrap();
                                        if let Some(line_num) = info.line {
                                            write!(line, ":{}", line_num).unwrap();
                                        }
                                        write!(line, "\x1b[m").unwrap();
                                    }
                                    println!("  {line}");
                                }
                                symbolize::Symbolized::Unknown(reason) => {
                                    eprintln!("    Unknown symbol: {reason}");
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Error symbolizing stack: {e}"),
                }
            }
            println!();
            st.stats.clear();
            st.stats_begin = std::time::Instant::now();
        }
    }
}

fn nul_termiate(s: &[u8]) -> &[u8] {
    let nul_pos = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    &s[..nul_pos]
}

/// Reverse‑join like `["foo","bar","baz"] → "baz/bar/foo"`.
fn rev_join(v: Vec<Vec<u8>>) -> Vec<u8> {
    v.into_iter()
        .rev()
        .flat_map(|s| std::iter::once(b'/').chain(s))
        .skip(1) // drop the very first ‘/’
        .collect()
}
