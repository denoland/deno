// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";

unitTest(async function permissionInvalidName(): Promise<void> {
  await assertThrowsAsync(async () => {
    // deno-lint-ignore no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  }, TypeError);
});

unitTest(async function permissionNetInvalidHost(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.permissions.query({ name: "net", host: ":" });
  }, URIError);
});

unitTest(async function permissionQueryReturnsEventTarget() {
  const status = await Deno.permissions.query({ name: "hrtime" });
  assert(["granted", "denied", "prompt"].includes(status.state));
  let called = false;
  status.addEventListener("change", () => {
    called = true;
  });
  status.dispatchEvent(new Event("change"));
  assert(called);
  assert(status === (await Deno.permissions.query({ name: "hrtime" })));
});

unitTest(async function permissionQueryForReadReturnsSameStatus() {
  const status1 = await Deno.permissions.query({
    name: "read",
    path: ".",
  });
  const status2 = await Deno.permissions.query({
    name: "read",
    path: ".",
  });
  assert(status1 === status2);
});

unitTest(function permissionsIllegalConstructor() {
  assertThrows(() => new Deno.Permissions(), TypeError, "Illegal constructor.");
  assertEquals(Deno.Permissions.length, 0);
});

unitTest(function permissionStatusIllegalConstructor() {
  assertThrows(
    () => new Deno.PermissionStatus(),
    TypeError,
    "Illegal constructor.",
  );
  assertEquals(Deno.PermissionStatus.length, 0);
});
