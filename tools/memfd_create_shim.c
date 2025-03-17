// Copyright 2018-2025 the Deno authors. MIT license.

#define _GNU_SOURCE
#include <sys/syscall.h>
#include <unistd.h>
#include <fcntl.h>

#ifndef SYS_memfd_create
#  if defined(__x86_64__)
#    define SYS_memfd_create 319
#  elif defined(__aarch64__)
#    define SYS_memfd_create 279
#  elif defined(__arm__)
#    define SYS_memfd_create 385
#  elif defined(__i386__)
#    define SYS_memfd_create 356
#  elif defined(__powerpc64__)
#    define SYS_memfd_create 360
#  else
#    error "memfd_create syscall number unknown for this architecture"
#  endif
#endif

int memfd_create(const char *name, unsigned int flags) {
  return syscall(SYS_memfd_create, name, flags);
}
