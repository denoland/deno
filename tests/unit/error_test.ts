// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assertEquals,
  assertInstanceOf,
  assertThrows,
  fail,
} from "@std/assert";

const errorConstructors = [
  Deno.errors.NotFound,
  Deno.errors.PermissionDenied,
  Deno.errors.ConnectionRefused,
  Deno.errors.ConnectionReset,
  Deno.errors.ConnectionAborted,
  Deno.errors.NotConnected,
  Deno.errors.AddrInUse,
  Deno.errors.AddrNotAvailable,
  Deno.errors.BrokenPipe,
  Deno.errors.AlreadyExists,
  Deno.errors.InvalidData,
  Deno.errors.TimedOut,
  Deno.errors.Interrupted,
  Deno.errors.WriteZero,
  Deno.errors.WouldBlock,
  Deno.errors.UnexpectedEof,
  Deno.errors.BadResource,
  Deno.errors.Http,
  Deno.errors.Busy,
  Deno.errors.NotSupported,
  Deno.errors.FilesystemLoop,
  Deno.errors.IsADirectory,
  Deno.errors.NetworkUnreachable,
  Deno.errors.NotADirectory,
  Deno.errors.NotCapable,
];

function assertError(ErrorConstructor: typeof Deno.errors.NotFound) {
  const error = new ErrorConstructor("msg", { cause: "cause" });
  assertInstanceOf(error, Error);
  assertEquals(error.cause, "cause");
}

Deno.test("Errors work", () => {
  for (const errorConstructor of errorConstructors) {
    assertError(errorConstructor);
  }
});

Deno.test("Errors have some tamper resistance", () => {
  // deno-lint-ignore no-explicit-any
  (Object.prototype as any).get = () => {};
  assertThrows(() => fail("test error"), Error, "test error");
  // deno-lint-ignore no-explicit-any
  delete (Object.prototype as any).get;
});
