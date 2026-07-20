// Copyright 2018-2026 the Deno authors. MIT license.
// Exact first-entry tracer for functions in a local macOS arm64 Mach-O executable.
//
// Page-granularity tracing can only say that some function on a 16 KiB page ran. This
// tracer replaces every LC_FUNCTION_STARTS entry in __text with a BRK, records
// the first trap at each entry, restores the original instruction, and resumes
// execution. The executable mapping is backed by a private shared-memory copy
// so the signal handler can restore instructions through a writable alias
// without requiring a writable+executable mapping.
//
// This is an offline profiling tool, not production runtime code. Its signal
// handler calls sys_icache_invalidate(), and the traced process executes a
// modified anonymous copy of __text.

#if !defined(__APPLE__) || !defined(__aarch64__)
#error "orderfile_function_tracer_macos.c requires arm64 macOS"
#endif

#define _DARWIN_C_SOURCE 1
#define _XOPEN_SOURCE 700

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <libkern/OSCacheControl.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <ucontext.h>
#include <unistd.h>

#define TRACE_MAGIC UINT64_C(0x44454e4f46554e43)
#define TRACE_VERSION UINT64_C(1)
#define HEADER_WORDS 5
#define BREAKPOINT_INSTRUCTION UINT32_C(0xd4200000)

typedef int (*sigaction_fn)(
  int,
  const struct sigaction *,
  struct sigaction *);

struct interpose_entry {
  const void *replacement;
  const void *replacee;
};

static int tracer_sigaction(
  int,
  const struct sigaction *,
  struct sigaction *);

__attribute__((used, section("__DATA,__interpose")))
static const struct interpose_entry sigaction_interpose = {
  (const void *)tracer_sigaction,
  (const void *)sigaction,
};

static sigaction_fn real_sigaction;
static struct sigaction previous_sigtrap;
static struct sigaction previous_sigill;
static uintptr_t image_slide;
static uintptr_t text_start;
static uintptr_t text_end;
static uintptr_t mapped_start;
static uintptr_t mapped_end;
static uint8_t *writable_alias;
static uintptr_t *function_starts;
static uint32_t *original_instructions;
static uint8_t *seen;
static size_t function_count;
static uint64_t *record;
static int armed;
static int debug_enabled;
static int text_is_writable_executable;

static void debug_message(const char *message) {
  if (debug_enabled) {
    write(STDERR_FILENO, message, strlen(message));
  }
}

static void debug_code(const char *operation, int code) {
  if (debug_enabled) {
    char message[256];
    int length = snprintf(
      message,
      sizeof(message),
      "function tracer: %s failed (%d)\n",
      operation,
      code);
    if (length > 0) {
      write(STDERR_FILENO, message, (size_t)length);
    }
  }
}

static uintptr_t align_down(uintptr_t value, uintptr_t alignment) {
  return value & ~(alignment - 1);
}

static uintptr_t align_up(uintptr_t value, uintptr_t alignment) {
  return (value + alignment - 1) & ~(alignment - 1);
}

static uint64_t read_uleb128(
  const uint8_t **cursor,
  const uint8_t *end,
  int *valid) {
  uint64_t value = 0;
  unsigned shift = 0;
  while (*cursor < end && shift < 64) {
    uint8_t byte = *(*cursor)++;
    value |= (uint64_t)(byte & 0x7f) << shift;
    if ((byte & 0x80) == 0) {
      *valid = 1;
      return value;
    }
    shift += 7;
  }
  *valid = 0;
  return 0;
}

static const struct mach_header_64 *find_main_image(uint32_t *image_index) {
  uint32_t count = _dyld_image_count();
  for (uint32_t index = 0; index < count; index++) {
    const struct mach_header_64 *header =
      (const struct mach_header_64 *)_dyld_get_image_header(index);
    if (
      header != NULL &&
      header->magic == MH_MAGIC_64 &&
      header->filetype == MH_EXECUTE
    ) {
      *image_index = index;
      return header;
    }
  }
  return NULL;
}

static int discover_functions(void) {
  uint32_t image_index = 0;
  const struct mach_header_64 *header = find_main_image(&image_index);
  if (header == NULL) {
    debug_message("function tracer: no MH_EXECUTE image\n");
    return -1;
  }
  image_slide = (uintptr_t)_dyld_get_image_vmaddr_slide(image_index);

  uint64_t text_vmaddr = 0;
  uint64_t linkedit_vmaddr = 0;
  uint64_t linkedit_fileoff = 0;
  uint32_t starts_dataoff = 0;
  uint32_t starts_datasize = 0;

  const uint8_t *command_bytes = (const uint8_t *)(header + 1);
  for (uint32_t index = 0; index < header->ncmds; index++) {
    const struct load_command *command =
      (const struct load_command *)command_bytes;
    if (command->cmdsize < sizeof(*command)) {
      return -1;
    }
    if (command->cmd == LC_SEGMENT_64) {
      const struct segment_command_64 *segment =
        (const struct segment_command_64 *)command;
      if (strncmp(segment->segname, SEG_TEXT, sizeof(segment->segname)) == 0) {
        text_vmaddr = segment->vmaddr;
        const struct section_64 *sections =
          (const struct section_64 *)(segment + 1);
        for (uint32_t section_index = 0;
             section_index < segment->nsects;
             section_index++) {
          const struct section_64 *section = &sections[section_index];
          if (
            strncmp(
              section->sectname,
              SECT_TEXT,
              sizeof(section->sectname)) == 0
          ) {
            text_start = (uintptr_t)section->addr + image_slide;
            text_end = text_start + (uintptr_t)section->size;
          }
        }
      } else if (
        strncmp(
          segment->segname,
          SEG_LINKEDIT,
          sizeof(segment->segname)) == 0
      ) {
        linkedit_vmaddr = segment->vmaddr;
        linkedit_fileoff = segment->fileoff;
      }
    } else if (command->cmd == LC_FUNCTION_STARTS) {
      const struct linkedit_data_command *starts =
        (const struct linkedit_data_command *)command;
      starts_dataoff = starts->dataoff;
      starts_datasize = starts->datasize;
    }
    command_bytes += command->cmdsize;
  }

  if (
    text_vmaddr == 0 ||
    text_start == 0 ||
    text_end <= text_start ||
    linkedit_vmaddr == 0 ||
    starts_dataoff == 0 ||
    starts_datasize == 0
  ) {
    debug_message("function tracer: incomplete Mach-O metadata\n");
    return -1;
  }

  const uint8_t *starts_begin = (const uint8_t *)(
    image_slide +
    (uintptr_t)linkedit_vmaddr +
    starts_dataoff -
    (uintptr_t)linkedit_fileoff);
  const uint8_t *starts_end = starts_begin + starts_datasize;

  // One byte is the smallest possible encoded delta, so datasize is a safe
  // upper bound for the number of starts.
  function_starts = calloc(starts_datasize, sizeof(*function_starts));
  if (function_starts == NULL) {
    return -1;
  }

  uintptr_t address = (uintptr_t)text_vmaddr + image_slide;
  const uint8_t *cursor = starts_begin;
  while (cursor < starts_end) {
    int valid = 0;
    uint64_t delta = read_uleb128(&cursor, starts_end, &valid);
    if (!valid) {
      return -1;
    }
    if (delta == 0) {
      break;
    }
    if (delta > UINTPTR_MAX - address) {
      return -1;
    }
    address += (uintptr_t)delta;
    if (
      address >= text_start &&
      address < text_end &&
      (address & (sizeof(uint32_t) - 1)) == 0
    ) {
      function_starts[function_count++] = address;
    }
  }

  if (function_count == 0) {
    debug_message("function tracer: LC_FUNCTION_STARTS was empty\n");
    return -1;
  }
  return 0;
}

static size_t find_function(uintptr_t address) {
  size_t low = 0;
  size_t high = function_count;
  while (low < high) {
    size_t middle = low + (high - low) / 2;
    uintptr_t candidate = function_starts[middle];
    if (candidate < address) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  if (low < function_count && function_starts[low] == address) {
    return low;
  }
  return SIZE_MAX;
}

static void restore_previous_handler(
  int signal_number,
  const struct sigaction *previous) {
  armed = 0;
  if (real_sigaction != NULL) {
    real_sigaction(signal_number, previous, NULL);
  }
}

static void on_breakpoint(
  int signal_number,
  siginfo_t *info,
  void *context_pointer) {
  (void)info;
  ucontext_t *context = (ucontext_t *)context_pointer;
  uintptr_t pc = (uintptr_t)context->uc_mcontext->__ss.__pc;
  uintptr_t function_address = pc;
  size_t index = find_function(function_address);
  if (index == SIZE_MAX && pc >= sizeof(uint32_t)) {
    function_address = pc - sizeof(uint32_t);
    index = find_function(function_address);
  }
  if (index == SIZE_MAX) {
    restore_previous_handler(
      signal_number,
      signal_number == SIGTRAP ? &previous_sigtrap : &previous_sigill);
    return;
  }

  uint32_t *writable_instruction = (uint32_t *)(
    writable_alias + function_address - mapped_start);
  __atomic_store_n(
    writable_instruction,
    original_instructions[index],
    __ATOMIC_RELEASE);
  sys_icache_invalidate((void *)function_address, sizeof(uint32_t));

  if (__atomic_exchange_n(&seen[index], 1, __ATOMIC_RELAXED) == 0) {
    uint64_t hit = __atomic_fetch_add(&record[4], 1, __ATOMIC_RELAXED);
    if (hit < function_count) {
      record[HEADER_WORDS + hit] = function_address - image_slide;
    }
  }

  // Darwin normally reports the BRK address itself. Also resetting the PC
  // makes the fallback pc-4 interpretation safe across kernel versions.
  context->uc_mcontext->__ss.__pc = function_address;
}

static int tracer_sigaction(
  int signal_number,
  const struct sigaction *action,
  struct sigaction *old_action) {
  if (real_sigaction == NULL) {
    real_sigaction = (sigaction_fn)sigaction_interpose.replacee;
  }
  if (armed && (signal_number == SIGTRAP || signal_number == SIGILL)) {
    if (old_action != NULL) {
      memset(old_action, 0, sizeof(*old_action));
    }
    return 0;
  }
  return real_sigaction(signal_number, action, old_action);
}

static int create_trace_output(const char *output) {
  size_t words = HEADER_WORDS + function_count;
  size_t bytes = words * sizeof(uint64_t);
  int fd = open(output, O_CREAT | O_TRUNC | O_RDWR, 0644);
  if (fd < 0 || ftruncate(fd, (off_t)bytes) != 0) {
    if (fd >= 0) {
      close(fd);
    }
    return -1;
  }
  record = mmap(NULL, bytes, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
  close(fd);
  if (record == MAP_FAILED) {
    record = NULL;
    return -1;
  }
  record[0] = TRACE_MAGIC;
  record[1] = TRACE_VERSION;
  record[2] = image_slide;
  record[3] = function_count;
  return 0;
}

static int create_executable_copy(uintptr_t page_size) {
  mapped_start = align_down(text_start, page_size);
  mapped_end = align_up(text_end, page_size);
  size_t bytes = mapped_end - mapped_start;

  original_instructions = calloc(
    function_count,
    sizeof(*original_instructions));
  seen = calloc(function_count, sizeof(*seen));
  if (original_instructions == NULL || seen == NULL) {
    return -1;
  }

  kern_return_t protection_result = mach_vm_protect(
    mach_task_self(),
    mapped_start,
    bytes,
    FALSE,
    VM_PROT_READ | VM_PROT_WRITE | VM_PROT_EXECUTE | VM_PROT_COPY);
  if (protection_result == KERN_SUCCESS) {
    text_is_writable_executable = 1;
    debug_message("function tracer: using writable executable COW text\n");
  } else {
    debug_code("writable executable mach_vm_protect", protection_result);
    protection_result = mach_vm_protect(
      mach_task_self(),
      mapped_start,
      bytes,
      FALSE,
      VM_PROT_READ | VM_PROT_WRITE | VM_PROT_COPY);
    if (protection_result != KERN_SUCCESS) {
      debug_code("writable text mach_vm_protect", protection_result);
      return -1;
    }
  }

  if (text_is_writable_executable) {
    writable_alias = (uint8_t *)mapped_start;
  } else {
    mach_vm_address_t alias_address = 0;
    vm_prot_t current_protection = VM_PROT_NONE;
    vm_prot_t maximum_protection = VM_PROT_NONE;
    kern_return_t remap_result = mach_vm_remap(
      mach_task_self(),
      &alias_address,
      bytes,
      0,
      VM_FLAGS_ANYWHERE,
      mach_task_self(),
      mapped_start,
      FALSE,
      &current_protection,
      &maximum_protection,
      VM_INHERIT_DEFAULT);
    if (remap_result != KERN_SUCCESS) {
      debug_code("writable alias mach_vm_remap", remap_result);
      return -1;
    }
    writable_alias = (uint8_t *)alias_address;
    if (
      mach_vm_protect(
        mach_task_self(),
        alias_address,
        bytes,
        FALSE,
        VM_PROT_READ | VM_PROT_WRITE) != KERN_SUCCESS
    ) {
      debug_message("function tracer: writable alias protection failed\n");
      return -1;
    }
  }

  for (size_t index = 0; index < function_count; index++) {
    uint32_t *instruction = (uint32_t *)(
      writable_alias + function_starts[index] - mapped_start);
    original_instructions[index] = *instruction;
    *instruction = BREAKPOINT_INSTRUCTION;
  }
  sys_icache_invalidate((void *)mapped_start, bytes);
  if (
    !text_is_writable_executable &&
    mach_vm_protect(
      mach_task_self(),
      mapped_start,
      bytes,
      FALSE,
      VM_PROT_READ | VM_PROT_EXECUTE) != KERN_SUCCESS
  ) {
    return -1;
  }
  return 0;
}

__attribute__((constructor(101)))
static void initialize_function_tracer(void) {
  debug_enabled = getenv("DENO_FUNCTION_TRACE_DEBUG") != NULL;
  const char *output_env = getenv("DENO_FUNCTION_TRACE_OUT");
  if (output_env == NULL) {
    return;
  }

  char output[4096];
  snprintf(output, sizeof(output), "%s", output_env);
  unsetenv("DYLD_INSERT_LIBRARIES");
  unsetenv("DENO_FUNCTION_TRACE_OUT");

  long reported_page_size = sysconf(_SC_PAGESIZE);
  if (reported_page_size <= 0) {
    debug_message("function tracer: invalid page size\n");
    return;
  }
  if (discover_functions() != 0) {
    debug_message("function tracer: discovery failed\n");
    return;
  }
  if (create_trace_output(output) != 0) {
    debug_message("function tracer: output setup failed\n");
    return;
  }

  static char alternate_stack[256 * 1024];
  stack_t stack = {
    .ss_sp = alternate_stack,
    .ss_size = sizeof(alternate_stack),
    .ss_flags = 0,
  };
  sigaltstack(&stack, NULL);

  real_sigaction = (sigaction_fn)sigaction_interpose.replacee;
  if (real_sigaction == NULL) {
    debug_message("function tracer: could not resolve sigaction\n");
    return;
  }
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_sigaction = on_breakpoint;
  action.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_NODEFER;
  sigemptyset(&action.sa_mask);
  real_sigaction(SIGTRAP, &action, &previous_sigtrap);
  real_sigaction(SIGILL, &action, &previous_sigill);

  if (create_executable_copy((uintptr_t)reported_page_size) != 0) {
    debug_message("function tracer: executable copy failed\n");
    return;
  }
  armed = 1;

  if (debug_enabled) {
    char message[256];
    int length = snprintf(
      message,
      sizeof(message),
      "function tracer: armed %zu starts in %#lx-%#lx\n",
      function_count,
      (unsigned long)text_start,
      (unsigned long)text_end);
    if (length > 0) {
      write(STDERR_FILENO, message, (size_t)length);
    }
  }
}
