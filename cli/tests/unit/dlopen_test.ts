// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

import { assertThrows, unitTest } from "./test_util.ts";

unitTest(function dlopenInvalidArguments() {
  const filename = "/usr/lib/libc.so.6";
  assertThrows(() => {
    Deno.dlopen(filename, { malloc: null } as any);
  }, TypeError);
  assertThrows(() => {
    Deno.dlopen(filename, {
      malloc: { parameters: ["a"], result: "b" },
    } as any);
  }, TypeError);
  assertThrows(() => {
    Deno.dlopen(filename, null as any);
  }, TypeError);
  assertThrows(() => {
    (Deno.dlopen as any)(filename);
  }, TypeError);
});
