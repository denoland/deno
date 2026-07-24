// Copyright 2018-2026 the Deno authors. MIT license.
// Exact first-entry tracer for functions in a local ELF executable.
//
// The generator supplies linker-visible STT_FUNC addresses from the
// unstripped executable. This tracer replaces the first instruction of every
// entry with INT3 on x86-64 or BRK on arm64, records the first trap, restores
// the instruction through a writable alias, rewinds the PC, and resumes
// execution.
//
// The executable PT_LOAD mapping is replaced with a shared memfd copy. It is
// mapped read+execute at the original addresses and read+write at a separate
// alias, avoiding a writable+executable mapping in the signal handler.
// This is an offline profiling tool, not production runtime code.

#if !defined(__linux__) || \
  (!defined(__x86_64__) && !defined(__aarch64__))
#error "orderfile_function_tracer_linux.c requires x86-64 or arm64 Linux"
#endif

#define _GNU_SOURCE

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <link.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <ucontext.h>
#include <unistd.h>

#define TRACE_MAGIC UINT64_C(0x44454e4f46554e43)
#define TRACE_VERSION UINT64_C(1)
#define HEADER_WORDS 5
#define STARTS_MAGIC UINT64_C(0x44454e4f53544152)
#define STARTS_VERSION UINT64_C(1)
#define STARTS_HEADER_WORDS 3
#define MAX_EXEC_REGIONS 8

#if defined(__x86_64__)
typedef uint8_t instruction_t;
#define BREAKPOINT_INSTRUCTION UINT8_C(0xcc)
#elif defined(__aarch64__)
typedef uint32_t instruction_t;
#define BREAKPOINT_INSTRUCTION UINT32_C(0xd4200000)
#endif

typedef int (*sigaction_fn)(
  int,
  const struct sigaction *,
  struct sigaction *);

struct exec_region {
  uintptr_t start;
  uintptr_t end;
  uint8_t *writable_alias;
  int fd;
};

static struct exec_region regions[MAX_EXEC_REGIONS];
static size_t region_count;
static uintptr_t image_slide;
static uintptr_t page_size;
static uintptr_t *function_starts;
static instruction_t *original_instructions;
static uint8_t *seen;
static size_t function_count;
static uint64_t *record;
static sigaction_fn real_sigaction;
static struct sigaction previous_sigtrap;
static int armed;
static int debug_enabled;
static uintptr_t last_function_address;

static void write_debug_bytes(const char *message, size_t length) {
  ssize_t result = write(STDERR_FILENO, message, length);
  (void)result;
}

static void debug_message(const char *message) {
  if (debug_enabled) {
    write_debug_bytes(message, strlen(message));
  }
}

static void debug_errno(const char *operation) {
  if (debug_enabled) {
    char message[256];
    int length = snprintf(
      message,
      sizeof(message),
      "function tracer: %s failed: %s\n",
      operation,
      strerror(errno));
    if (length > 0) {
      write_debug_bytes(message, (size_t)length);
    }
  }
}

static uintptr_t align_down(uintptr_t value, uintptr_t alignment) {
  return value & ~(alignment - 1);
}

static uintptr_t align_up(uintptr_t value, uintptr_t alignment) {
  return (value + alignment - 1) & ~(alignment - 1);
}

static int compare_uintptr(const void *left_pointer, const void *right_pointer) {
  uintptr_t left = *(const uintptr_t *)left_pointer;
  uintptr_t right = *(const uintptr_t *)right_pointer;
  return left < right ? -1 : left > right;
}

static int discover_main_image(
  struct dl_phdr_info *info,
  size_t size,
  void *data) {
  (void)size;
  (void)data;
  if (info->dlpi_name[0] != '\0') {
    return 0;
  }

  image_slide = (uintptr_t)info->dlpi_addr;
  for (ElfW(Half) index = 0; index < info->dlpi_phnum; index++) {
    const ElfW(Phdr) *header = &info->dlpi_phdr[index];
    if (
      header->p_type != PT_LOAD ||
      (header->p_flags & PF_X) == 0 ||
      region_count == MAX_EXEC_REGIONS
    ) {
      continue;
    }
    uintptr_t start = image_slide + (uintptr_t)header->p_vaddr;
    uintptr_t end = start + (uintptr_t)header->p_memsz;
    regions[region_count].start = align_down(start, page_size);
    regions[region_count].end = align_up(end, page_size);
    regions[region_count].fd = -1;
    region_count++;
  }
  return 1;
}

static size_t region_for_address(uintptr_t address) {
  for (size_t index = 0; index < region_count; index++) {
    if (address >= regions[index].start && address < regions[index].end) {
      return index;
    }
  }
  return SIZE_MAX;
}

static void synchronize_instruction_cache(
  uintptr_t executable_address,
  uint8_t *writable_address,
  size_t bytes) {
#if defined(__aarch64__)
  // The modified memfd is visible through different writable and executable
  // virtual addresses. Clean the data cache through the writable alias, then
  // invalidate the instruction cache through the address execution uses.
  __builtin___clear_cache(
    (char *)writable_address,
    (char *)writable_address + bytes);
  __builtin___clear_cache(
    (char *)executable_address,
    (char *)executable_address + bytes);
#else
  (void)executable_address;
  (void)writable_address;
  (void)bytes;
#endif
}

static int read_function_starts(const char *path) {
  int fd = open(path, O_RDONLY | O_CLOEXEC);
  if (fd < 0) {
    debug_errno("open starts file");
    return -1;
  }
  struct stat status;
  if (fstat(fd, &status) != 0 || status.st_size < STARTS_HEADER_WORDS * 8) {
    close(fd);
    return -1;
  }
  size_t bytes = (size_t)status.st_size;
  const uint64_t *words = mmap(NULL, bytes, PROT_READ, MAP_PRIVATE, fd, 0);
  close(fd);
  if (words == MAP_FAILED) {
    debug_errno("map starts file");
    return -1;
  }
  uint64_t count = words[2];
  if (
    words[0] != STARTS_MAGIC ||
    words[1] != STARTS_VERSION ||
    count == 0 ||
    count > SIZE_MAX / sizeof(uintptr_t) ||
    count > (bytes / 8) - STARTS_HEADER_WORDS
  ) {
    munmap((void *)words, bytes);
    return -1;
  }

  function_starts = calloc((size_t)count, sizeof(*function_starts));
  if (function_starts == NULL) {
    munmap((void *)words, bytes);
    return -1;
  }
  for (size_t index = 0; index < (size_t)count; index++) {
    uint64_t link_address = words[STARTS_HEADER_WORDS + index];
    if (link_address <= UINTPTR_MAX - image_slide) {
      function_starts[function_count++] =
        image_slide + (uintptr_t)link_address;
    }
  }
  munmap((void *)words, bytes);
  qsort(
    function_starts,
    function_count,
    sizeof(*function_starts),
    compare_uintptr);

  size_t unique_count = 0;
  for (size_t index = 0; index < function_count; index++) {
    uintptr_t address = function_starts[index];
    if (
      region_for_address(address) != SIZE_MAX &&
      (unique_count == 0 || function_starts[unique_count - 1] != address)
    ) {
      function_starts[unique_count++] = address;
    }
  }
  function_count = unique_count;
  if (function_count == 0) {
    debug_message("function tracer: starts file had no executable entries\n");
    return -1;
  }
  return 0;
}

static int create_memfd(const char *name) {
#ifdef SYS_memfd_create
  return (int)syscall(SYS_memfd_create, name, MFD_CLOEXEC);
#else
  (void)name;
  errno = ENOSYS;
  return -1;
#endif
}

static int copy_executable_regions(void) {
  for (size_t index = 0; index < region_count; index++) {
    struct exec_region *region = &regions[index];
    size_t bytes = region->end - region->start;
    region->fd = create_memfd("deno-function-trace");
    if (region->fd < 0 || ftruncate(region->fd, (off_t)bytes) != 0) {
      debug_errno("create executable memfd");
      return -1;
    }
    region->writable_alias = mmap(
      NULL,
      bytes,
      PROT_READ | PROT_WRITE,
      MAP_SHARED,
      region->fd,
      0);
    if (region->writable_alias == MAP_FAILED) {
      region->writable_alias = NULL;
      debug_errno("map writable alias");
      return -1;
    }
    memcpy(region->writable_alias, (const void *)region->start, bytes);
  }
  return 0;
}

static int install_breakpoints(void) {
  original_instructions = calloc(
    function_count,
    sizeof(*original_instructions));
  seen = calloc(function_count, sizeof(*seen));
  if (original_instructions == NULL || seen == NULL) {
    return -1;
  }
  size_t patchable_count = 0;
  for (size_t index = 0; index < function_count; index++) {
    uintptr_t address = function_starts[index];
    size_t region_index = region_for_address(address);
    if (region_index == SIZE_MAX) {
      continue;
    }
    struct exec_region *region = &regions[region_index];
    if (
      address % sizeof(instruction_t) != 0 ||
      address > region->end - sizeof(instruction_t)
    ) {
      continue;
    }
    instruction_t *instruction = (instruction_t *)(
      region->writable_alias + address - region->start);
    instruction_t original = __atomic_load_n(
      instruction,
      __ATOMIC_RELAXED);
    if (original == BREAKPOINT_INSTRUCTION) {
      continue;
    }
    function_starts[patchable_count] = address;
    original_instructions[patchable_count] = original;
    patchable_count++;
    __atomic_store_n(
      instruction,
      BREAKPOINT_INSTRUCTION,
      __ATOMIC_RELAXED);
  }
  function_count = patchable_count;
  if (function_count == 0) {
    return -1;
  }
  return 0;
}

static int replace_executable_regions(void) {
  for (size_t index = 0; index < region_count; index++) {
    struct exec_region *region = &regions[index];
    size_t bytes = region->end - region->start;
    synchronize_instruction_cache(
      region->start,
      region->writable_alias,
      bytes);
    void *mapping = mmap(
      (void *)region->start,
      bytes,
      PROT_READ | PROT_EXEC,
      MAP_SHARED | MAP_FIXED,
      region->fd,
      0);
    if (mapping == MAP_FAILED) {
      debug_errno("replace executable mapping");
      return -1;
    }
    synchronize_instruction_cache(
      region->start,
      region->writable_alias,
      bytes);
    close(region->fd);
    region->fd = -1;
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

static void forward_sigtrap(
  int signal_number,
  siginfo_t *info,
  void *context_pointer) {
  if (previous_sigtrap.sa_handler == SIG_IGN) {
    return;
  }
  if (previous_sigtrap.sa_handler == SIG_DFL) {
    armed = 0;
    if (real_sigaction != NULL) {
      real_sigaction(signal_number, &previous_sigtrap, NULL);
    }
    kill(getpid(), signal_number);
    return;
  }
  if ((previous_sigtrap.sa_flags & SA_SIGINFO) != 0) {
    previous_sigtrap.sa_sigaction(signal_number, info, context_pointer);
  } else {
    previous_sigtrap.sa_handler(signal_number);
  }
}

static void on_breakpoint(
  int signal_number,
  siginfo_t *info,
  void *context_pointer) {
  ucontext_t *context = (ucontext_t *)context_pointer;
#if defined(__x86_64__)
  uintptr_t pc = (uintptr_t)context->uc_mcontext.gregs[REG_RIP];
#elif defined(__aarch64__)
  uintptr_t pc = (uintptr_t)context->uc_mcontext.pc;
#endif
  if (pc == 0) {
    forward_sigtrap(signal_number, info, context_pointer);
    return;
  }
#if defined(__x86_64__)
  uintptr_t function_address = pc - sizeof(instruction_t);
  size_t index = find_function(function_address);
#elif defined(__aarch64__)
  uintptr_t function_address = pc;
  size_t index = find_function(function_address);
  if (index == SIZE_MAX && pc >= sizeof(instruction_t)) {
    function_address = pc - sizeof(instruction_t);
    index = find_function(function_address);
  }
#endif
  if (index == SIZE_MAX) {
    if (debug_enabled) {
      char message[256];
      int length = snprintf(
        message,
        sizeof(message),
        "function tracer: unknown SIGTRAP at pc=%#lx candidate=%#lx "
        "code=%d hits=%lu last=%#lx\n",
        (unsigned long)pc,
        (unsigned long)function_address,
        info->si_code,
        record == NULL ? 0 : (unsigned long)record[4],
        (unsigned long)last_function_address);
      if (length > 0) {
        write_debug_bytes(message, (size_t)length);
      }
    }
    forward_sigtrap(signal_number, info, context_pointer);
    return;
  }

  size_t region_index = region_for_address(function_address);
  if (region_index == SIZE_MAX) {
    forward_sigtrap(signal_number, info, context_pointer);
    return;
  }
  struct exec_region *region = &regions[region_index];
  instruction_t *writable_instruction = (instruction_t *)(
    region->writable_alias + function_address - region->start);
  __atomic_store_n(
    writable_instruction,
    original_instructions[index],
    __ATOMIC_RELEASE);
  synchronize_instruction_cache(
    function_address,
    (uint8_t *)writable_instruction,
    sizeof(*writable_instruction));
  last_function_address = function_address;

  if (__atomic_exchange_n(&seen[index], 1, __ATOMIC_RELAXED) == 0) {
    uint64_t hit = __atomic_fetch_add(&record[4], 1, __ATOMIC_RELAXED);
    if (hit < function_count) {
      record[HEADER_WORDS + hit] = function_address - image_slide;
    }
  }
#if defined(__x86_64__)
  context->uc_mcontext.gregs[REG_RIP] = (greg_t)function_address;
#elif defined(__aarch64__)
  context->uc_mcontext.pc = function_address;
#endif
}

int sigaction(
  int signal_number,
  const struct sigaction *action,
  struct sigaction *old_action) {
  if (real_sigaction == NULL) {
    real_sigaction = (sigaction_fn)dlsym(RTLD_NEXT, "sigaction");
  }
  if (armed && signal_number == SIGTRAP) {
    if (old_action != NULL) {
      *old_action = previous_sigtrap;
    }
    if (action != NULL) {
      previous_sigtrap = *action;
      if (debug_enabled) {
        char message[256];
        int length = snprintf(
          message,
          sizeof(message),
          "function tracer: retained SIGTRAP handler=%p flags=%#x\n",
          (void *)previous_sigtrap.sa_handler,
          previous_sigtrap.sa_flags);
        if (length > 0) {
          write_debug_bytes(message, (size_t)length);
        }
      }
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

__attribute__((constructor(101)))
static void initialize_function_tracer(void) {
  debug_enabled = getenv("DENO_FUNCTION_TRACE_DEBUG") != NULL;
  const char *output_env = getenv("DENO_FUNCTION_TRACE_OUT");
  const char *starts_env = getenv("DENO_FUNCTION_TRACE_STARTS");
  if (output_env == NULL || starts_env == NULL) {
    return;
  }

  char output[4096];
  char starts[4096];
  snprintf(output, sizeof(output), "%s", output_env);
  snprintf(starts, sizeof(starts), "%s", starts_env);
  unsetenv("LD_PRELOAD");
  unsetenv("DENO_FUNCTION_TRACE_OUT");
  unsetenv("DENO_FUNCTION_TRACE_STARTS");

  long reported_page_size = sysconf(_SC_PAGESIZE);
  if (reported_page_size <= 0) {
    debug_message("function tracer: invalid page size\n");
    return;
  }
  page_size = (uintptr_t)reported_page_size;
  dl_iterate_phdr(discover_main_image, NULL);
  if (region_count == 0) {
    debug_message("function tracer: no executable PT_LOAD mapping\n");
    return;
  }
  if (read_function_starts(starts) != 0) {
    debug_message("function tracer: function discovery failed\n");
    return;
  }
  if (copy_executable_regions() != 0 || install_breakpoints() != 0) {
    debug_message("function tracer: executable copy failed\n");
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

  real_sigaction = (sigaction_fn)dlsym(RTLD_NEXT, "sigaction");
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

  if (replace_executable_regions() != 0) {
    _exit(127);
  }
  armed = 1;

  if (debug_enabled) {
    char message[256];
    int length = snprintf(
      message,
      sizeof(message),
      "function tracer: armed %zu starts in %zu executable regions; "
      "handler=%p main=%#lx-%#lx previous=%p flags=%#x\n",
      function_count,
      region_count,
      (void *)on_breakpoint,
      (unsigned long)regions[0].start,
      (unsigned long)regions[region_count - 1].end,
      (void *)previous_sigtrap.sa_handler,
      previous_sigtrap.sa_flags);
    if (length > 0) {
      write_debug_bytes(message, (size_t)length);
    }
  }
}
