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

  // ---------------------------------
  // Infer: Parameter Types
  // ---------------------------------
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["usize", "usize"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(0);
    remote.symbols.method(0, 0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["void"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(void 0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["usize"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["isize"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["u8"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["u16"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["u32"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["u64"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["i8"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["i16"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["i32"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["i64"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["f32"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["f64"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(0);
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: ["pointer"], result: "void" },
      } as const,
    );
    // @ts-expect-error: Invalid argument
    remote.symbols.method(null);
    remote.symbols.method(new Uint16Array(1));
    remote.symbols.method({} as Deno.UnsafePointer);
  }, TypeError);
  // ---------------------------------
  // Infer: Return Type
  // ---------------------------------
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: [], result: "usize" },
      } as const,
    );
    const result = remote.symbols.method();
    // @ts-expect-error: Invalid argument
    const _0: string = result;
    const _1: number = result;
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: [], result: "usize", nonblocking: true },
      } as const,
    );
    const result = remote.symbols.method();
    // @ts-expect-error: Invalid argument
    result.then((_0: string) => {});
    result.then((_1: number) => {});
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: [], result: "pointer" },
      } as const,
    );
    const result = remote.symbols.method();
    // @ts-expect-error: Invalid argument
    const _0: Deno.TypedArray = result;
    const _1: Deno.UnsafePointer = result;
  }, TypeError);
  assertThrows(() => {
    const remote = Deno.dlopen(
      filename,
      {
        method: { parameters: [], result: "pointer", nonblocking: true },
      } as const,
    );
    const result = remote.symbols.method();
    // @ts-expect-error: Invalid argument
    result.then((_0: Deno.TypedArray) => {});
    result.then((_1: Deno.UnsafePointer) => {});
  }, TypeError);
});
