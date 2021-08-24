// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assertThrows, unitTest } from "./test_util.ts";

unitTest(function dlopenInvalidArguments() {
  const filename = "/usr/lib/libc.so.6";
  assertThrows(() => {
    // @ts-expect-error: ForeignFunction cannot be null
    Deno.dlopen(filename, { malloc: null });
  }, TypeError);
  assertThrows(() => {
    Deno.dlopen(filename, {
      // @ts-expect-error: invalid NativeType
      malloc: { parameters: ["a"], result: "b" },
    });
  }, TypeError);
  assertThrows(() => {
    // @ts-expect-error: DynamicLibrary symbols cannot be null
    Deno.dlopen(filename, null);
  }, TypeError);
  assertThrows(() => {
    // @ts-expect-error: require 2 arguments
    Deno.dlopen(filename);
  }, TypeError);
});
