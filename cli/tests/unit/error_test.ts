// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows, fail } from "./test_util.ts";

Deno.test("Errors work", () => {
  assert(new Deno.errors.NotFound("msg") instanceof Error);
  assert(new Deno.errors.PermissionDenied("msg") instanceof Error);
  assert(new Deno.errors.ConnectionRefused("msg") instanceof Error);
  assert(new Deno.errors.ConnectionReset("msg") instanceof Error);
  assert(new Deno.errors.ConnectionAborted("msg") instanceof Error);
  assert(new Deno.errors.NotConnected("msg") instanceof Error);
  assert(new Deno.errors.AddrInUse("msg") instanceof Error);
  assert(new Deno.errors.AddrNotAvailable("msg") instanceof Error);
  assert(new Deno.errors.BrokenPipe("msg") instanceof Error);
  assert(new Deno.errors.AlreadyExists("msg") instanceof Error);
  assert(new Deno.errors.InvalidData("msg") instanceof Error);
  assert(new Deno.errors.TimedOut("msg") instanceof Error);
  assert(new Deno.errors.Interrupted("msg") instanceof Error);
  assert(new Deno.errors.WriteZero("msg") instanceof Error);
  assert(new Deno.errors.UnexpectedEof("msg") instanceof Error);
  assert(new Deno.errors.BadResource("msg") instanceof Error);
  assert(new Deno.errors.Http("msg") instanceof Error);
  assert(new Deno.errors.Busy("msg") instanceof Error);
  assert(new Deno.errors.NotSupported("msg") instanceof Error);
});

Deno.test("Errors have some tamper resistance", () => {
  // deno-lint-ignore no-explicit-any
  (Object.prototype as any).get = () => {};
  assertThrows(() => fail("test error"), Error, "test error");
  // deno-lint-ignore no-explicit-any
  delete (Object.prototype as any).get;
});
