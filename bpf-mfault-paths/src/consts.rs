#![allow(
    dead_code,
    reason = "Copied from Linux kernel headers, not all constants are used in this crate"
)]

// https://elixir.bootlin.com/linux/v6.11.5/source/include/uapi/linux/limits.h#L12
pub const NAME_MAX: usize = 255;

// https://elixir.bootlin.com/linux/v6.11.5/source/include/linux/mm.h#L265-L308

pub const VM_NONE: u32 = 0x00000000;

pub const VM_READ: u32 = 0x00000001; // currently active flags
pub const VM_WRITE: u32 = 0x00000002;
pub const VM_EXEC: u32 = 0x00000004;
pub const VM_SHARED: u32 = 0x00000008;

pub const VM_MAYREAD: u32 = 0x00000010; // limits for mprotect() etc
pub const VM_MAYWRITE: u32 = 0x00000020;
pub const VM_MAYEXEC: u32 = 0x00000040;
pub const VM_MAYSHARE: u32 = 0x00000080;

pub const VM_GROWSDOWN: u32 = 0x00000100; // general info on the segment
pub const VM_PFNMAP: u32 = 0x00000400; // Page-ranges managed without "struct page", just pure PFN
pub const VM_UFFD_WP: u32 = 0x00001000; // wrprotect pages tracking

pub const VM_LOCKED: u32 = 0x00002000;
pub const VM_IO: u32 = 0x00004000; // Memory mapped I/O or similar

pub const VM_SEQ_READ: u32 = 0x00008000; // App will access data sequentially
pub const VM_RAND_READ: u32 = 0x00010000; // App will not benefit from clustered reads

pub const VM_DONTCOPY: u32 = 0x00020000; // Do not copy this vma on fork
pub const VM_DONTEXPAND: u32 = 0x00040000; // Cannot expand with mremap()
pub const VM_LOCKONFAULT: u32 = 0x00080000; // Lock the pages covered when they are faulted in
pub const VM_ACCOUNT: u32 = 0x00100000; // Is a VM accounted object
pub const VM_NORESERVE: u32 = 0x00200000; // should the VM suppress accounting
pub const VM_HUGETLB: u32 = 0x00400000; // Huge TLB Page VM
pub const VM_SYNC: u32 = 0x00800000; // Synchronous page faults
pub const VM_ARCH_1: u32 = 0x01000000; // Architecture-specific flag
pub const VM_WIPEONFORK: u32 = 0x02000000; // Wipe VMA contents in child.
pub const VM_DONTDUMP: u32 = 0x04000000; // Do not include in the core dump
