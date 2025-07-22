#ifndef __MMFAULT_PATHS_H
#define __MMFAULT_PATHS_H

#define NAME_MAX 255

enum event_type {
  EVENT_INODE_PATH = 0,
  EVENT_PAGE_FAULT = 1,
};

struct data_inode_path {
  __u64 inode;
  char path[NAME_MAX];
};

struct data_page_fault {
  __u32 pid;
  __u32 tid;

  __u32 vm_flags;
  __u32 fault_flags;

  __u64 inode;
  __u64 off;
  // struct bpf_stack_build_id stack[16];

  __u32 stack_size;
  __u64 stack[16];
};

struct data_t {
  __u64 type;
  union {
    struct data_inode_path inode_path;
    struct data_page_fault page_fault;
  };
};

#endif
