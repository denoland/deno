// Copyright 2018-2025 the Deno authors. MIT license.
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
  assert(new Deno.errors.WouldBlock("msg") instanceof Error);
  assert(new Deno.errors.WriteZero("msg") instanceof Error);
  assert(new Deno.errors.UnexpectedEof("msg") instanceof Error);
  assert(new Deno.errors.BadResource("msg") instanceof Error);
  assert(new Deno.errors.Http("msg") instanceof Error);
  assert(new Deno.errors.Busy("msg") instanceof Error);
  assert(new Deno.errors.NotSupported("msg") instanceof Error);
  assert(new Deno.errors.NotCapable("msg") instanceof Error);
});

Deno.test("Errors have some tamper resistance", () => {
  // deno-lint-ignore no-explicit-any
  (Object.prototype as any).get = () => {};
  assertThrows(() => fail("test error"), Error, "test error");
  // deno-lint-ignore no-explicit-any
  delete (Object.prototype as any).get;
});

Deno.test("System errors have optional code property", () => {
  // OS-level errors should have the code property
  const notFound = new Deno.errors.NotFound("test");
  assert(notFound instanceof Error);
  // TypeScript should accept .code without type assertion
  assert(notFound.code === undefined || typeof notFound.code === "string");

  const permissionDenied = new Deno.errors.PermissionDenied("test");
  assert(
    permissionDenied.code === undefined ||
      typeof permissionDenied.code === "string",
  );

  const connectionRefused = new Deno.errors.ConnectionRefused("test");
  assert(
    connectionRefused.code === undefined ||
      typeof connectionRefused.code === "string",
  );

  // Deno-specific errors should NOT have the code property
  const invalidData = new Deno.errors.InvalidData("test");
  assert(invalidData instanceof Error);
  // @ts-expect-error code should not exist on InvalidData
  assert(invalidData.code === undefined);

  const badResource = new Deno.errors.BadResource("test");
  // @ts-expect-error code should not exist on BadResource
  assert(badResource.code === undefined);

  const notCapable = new Deno.errors.NotCapable("test");
  // @ts-expect-error code should not exist on NotCapable
  assert(notCapable.code === undefined);
});
