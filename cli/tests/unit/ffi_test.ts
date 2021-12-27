// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assertThrows } from "./test_util.ts";

Deno.test({ permissions: { ffi: true } }, function dlopenInvalidArguments() {
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
  assertThrows(() => {
    const remote = Deno.dlopen(filename, {
      method: { parameters: [
        "usize", "isize", 
        "u8", "u16", "u32", "u64", 
        "i8", "i16", "i32", "i64", 
        "void", "pointer"
      ], result: "void" }
    } as const);
    remote.symbols.method(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, void 0, new Uint8Array(1));
    remote.symbols.method(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, void 0, {} as Deno.UnsafePointer);
    // @ts-expect-error: invalid arguments
    remote.symbols.method("0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0");
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(filename, {
      method: { parameters: [], result: "f32" }
    } as const);
    // @ts-expect-error Return type number | f32 is not assignable to type string
    const _: string = remote.symbols.method();
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(filename, {
      method: { parameters: [], result: "f32", nonblocking: true }
    } as const);
    // @ts-expect-error Return type Promise<number> | f32 is not assignable to type string
    remote.symbols.method().then((_: string) => {});
  }, TypeError);
});
