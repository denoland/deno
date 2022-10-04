// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
});

Deno.test({ permissions: { ffi: false } }, function ffiPermissionDenied() {
  assertThrows(() => {
    Deno.dlopen("/usr/lib/libc.so.6", {});
  }, Deno.errors.PermissionDenied);
  const fnptr = new Deno.UnsafeFnPointer(
    0n,
    {
      parameters: ["u32", "pointer"],
      result: "void",
    } as const,
  );
  assertThrows(() => {
    fnptr.call(123, null);
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    Deno.UnsafePointer.of(new Uint8Array(0));
  }, Deno.errors.PermissionDenied);
  const ptrView = new Deno.UnsafePointerView(0n);
  assertThrows(() => {
    ptrView.copyInto(new Uint8Array(0));
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getCString();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getUint8();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getInt8();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getUint16();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getInt16();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getUint32();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getInt32();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getFloat32();
  }, Deno.errors.PermissionDenied);
  assertThrows(() => {
    ptrView.getFloat64();
  }, Deno.errors.PermissionDenied);
});
