// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// From https://github.com/just-js/benchmarks/tree/main/01-stdio

#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>

int main(int argc, char *argv[]) {
  unsigned int blocksize = 65536;
  if (argc == 2) {
    blocksize = atoi(argv[1]);
  }
  char buf[blocksize];
  unsigned long size = 0;
  unsigned int reads = 0;
  int n = read(STDIN_FILENO, buf, blocksize);
  while (n > 0) {
    reads++;
    size += n;
    n = read(STDIN_FILENO, buf, blocksize);
  }
  if (n < 0) {
    fprintf(stderr, "read: %s (%i)\n", strerror(errno), errno);
    exit(1);
  }
  fprintf(stdout, "size %lu reads %u blocksize %u\n", size, reads, blocksize);
}
