// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

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
});

Deno.test({ permissions: { ffi: false } }, function ffiNotCapable() {
  assertThrows(() => {
    Deno.dlopen("/usr/lib/libc.so.6", {});
  }, Deno.errors.NotCapable);
  const fnptr = new Deno.UnsafeFnPointer(
    // @ts-expect-error: Not NonNullable but null check is after permissions check.
    null,
    {
      parameters: ["u32", "pointer"],
      result: "void",
    } as const,
  );
  assertThrows(() => {
    fnptr.call(123, null);
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    Deno.UnsafePointer.of(new Uint8Array(0));
  }, Deno.errors.NotCapable);
  const ptrView = new Deno.UnsafePointerView(
    // @ts-expect-error: Not NonNullable but null check is after permissions check.
    null,
  );
  assertThrows(() => {
    ptrView.copyInto(new Uint8Array(0));
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getCString();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getUint8();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getInt8();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getUint16();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getInt16();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getUint32();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getInt32();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getFloat32();
  }, Deno.errors.NotCapable);
  assertThrows(() => {
    ptrView.getFloat64();
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { ffi: true } }, function pointerOf() {
  const buffer = new ArrayBuffer(1024);
  const baseAddress = Deno.UnsafePointer.value(Deno.UnsafePointer.of(buffer));
  const uint8Address = Deno.UnsafePointer.value(
    Deno.UnsafePointer.of(new Uint8Array(buffer)),
  );
  assertEquals(baseAddress, uint8Address);
  const float64Address = Deno.UnsafePointer.value(
    Deno.UnsafePointer.of(new Float64Array(buffer)),
  );
  assertEquals(baseAddress, float64Address);
  const uint8AddressOffset = Deno.UnsafePointer.value(
    Deno.UnsafePointer.of(new Uint8Array(buffer, 100)),
  );
  assertEquals(baseAddress + 100n, uint8AddressOffset);
  const float64AddressOffset = Deno.UnsafePointer.value(
    Deno.UnsafePointer.of(new Float64Array(buffer, 80)),
  );
  assertEquals(baseAddress + 80n, float64AddressOffset);
});

Deno.test({ permissions: { ffi: true } }, function callWithError() {
  const throwCb = () => {
    throw new Error("Error");
  };
  const cb = new Deno.UnsafeCallback({
    parameters: [],
    result: "void",
  }, throwCb);
  const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, {
    parameters: [],
    result: "void",
  });
  assertThrows(() => fnPointer.call());
  cb.close();
});

Deno.test(
  { permissions: { ffi: true }, ignore: true },
  async function callNonBlockingWithError() {
    const throwCb = () => {
      throw new Error("Error");
    };
    const cb = new Deno.UnsafeCallback({
      parameters: [],
      result: "void",
    }, throwCb);
    const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, {
      parameters: [],
      result: "void",
      nonblocking: true,
    });
    // TODO(mmastrac): currently ignored as we do not thread callback exceptions through nonblocking pointers
    await assertRejects(async () => await fnPointer.call());
    cb.close();
  },
);
