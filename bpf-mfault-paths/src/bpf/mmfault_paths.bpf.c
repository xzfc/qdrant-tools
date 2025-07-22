#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#include "mmfault_paths.h"

#define PAGE_SHIFT 12
#define PAGE_SIZE (1UL << PAGE_SHIFT)
#define PAGE_MASK (~(PAGE_SIZE - 1))

/* Configurable PID to trace (0 = trace all) */
const volatile pid_t target_pid = 0;

struct {
  __uint(type, BPF_MAP_TYPE_LRU_PERCPU_HASH);
  __uint(max_entries, 32);
  __type(key, unsigned long);
  __type(value, __u8);
} last_dumped_paths SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
  __uint(key_size, sizeof(__u32));
  __uint(value_size, sizeof(__u32));
} events SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, struct data_t);
} scratch_buffer SEC(".maps");

static __always_inline void dump_path(struct pt_regs *ctx, struct data_t *data,
                                      struct dentry *dentry) {
  struct inode *leaf_inode = BPF_CORE_READ(dentry, d_inode);
  unsigned long root_ino = leaf_inode ? BPF_CORE_READ(leaf_inode, i_ino) : 0;

  // Don't dump the same inode more than once
  __u8 one = 1;
  if (bpf_map_lookup_elem(&last_dumped_paths, &root_ino))
    return;
  bpf_map_update_elem(&last_dumped_paths, &root_ino, &one, BPF_ANY);

  data->type = EVENT_INODE_PATH;
  data->inode_path.inode = root_ino;

#pragma unroll
  for (int i = 0; i < 10; i++) {
    if (!dentry)
      break;

    struct qstr d_name = BPF_CORE_READ(dentry, d_name);

    bpf_probe_read_kernel_str(&data->inode_path.path,
                              sizeof data->inode_path.path, d_name.name);
    bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, data, sizeof(*data));

    struct dentry *parent = BPF_CORE_READ(dentry, d_parent);
    if (parent == dentry)
      break;
    dentry = parent;
  }
}

SEC("kprobe/handle_mm_fault")
int BPF_KPROBE(trace_handle_mm_fault, struct vm_area_struct *vma,
               unsigned long address, unsigned int flags) {
  if (target_pid && ((__u32)(bpf_get_current_pid_tgid() >> 32)) != target_pid)
    return 0;

  struct file *file = BPF_CORE_READ(vma, vm_file);
  if (!file)
    return 0;

  struct dentry *dentry = BPF_CORE_READ(file, f_path.dentry);
  if (!dentry)
    return 0;

  __u32 zero = 0;
  struct data_t *data = bpf_map_lookup_elem(&scratch_buffer, &zero);
  if (!data)
    return 0;

  /* Align address to page boundary */
  address &= PAGE_MASK;
  unsigned long vm_start = BPF_CORE_READ(vma, vm_start);
  unsigned long pgoff = BPF_CORE_READ(vma, vm_pgoff);
  __u64 off = (address - vm_start) + ((__u64)pgoff << PAGE_SHIFT);

  dump_path(ctx, data, dentry);

  /* Emit PAGE_FAULT event */
  struct inode *inode = BPF_CORE_READ(dentry, d_inode);
  __u64 ino = inode ? BPF_CORE_READ(inode, i_ino) : 0;

  data->type = EVENT_PAGE_FAULT;
  data->page_fault = (struct data_page_fault){
      .pid = bpf_get_current_pid_tgid() >> 32,
      .tid = bpf_get_current_pid_tgid() & 0xFFFFFFFF,
      .vm_flags = BPF_CORE_READ(vma, vm_flags),
      .fault_flags = flags,
      .inode = ino,
      .off = off,
  };
  long stack_size =
      bpf_get_stack(ctx, data->page_fault.stack, sizeof data->page_fault.stack,
                    BPF_F_USER_STACK);
  data->page_fault.stack_size = stack_size < 0 ? 0 : stack_size / sizeof(__u64);

  bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, data, sizeof *data);

  return 0;
}

char LICENSE[] SEC("license") = "GPL";
