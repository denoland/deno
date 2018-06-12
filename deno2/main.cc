// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <assert.h>
#include <stdio.h>

#include "include/deno.h"

int main(int argc, char** argv) {
  deno_init();

  Deno* d = deno_new(NULL, NULL);
  int r = deno_load(d, "main2.js", "foo();");
  if (r != 0) {
    printf("Error! %s\n", deno_last_exception(d));
    exit(1);
  }
}
