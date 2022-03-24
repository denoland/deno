// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

Deno.test(async function permissionInvalidName() {
  await assertRejects(async () => {
    // deno-lint-ignore no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  }, TypeError);
});

Deno.test(async function permissionNetInvalidHost() {
  await assertRejects(async () => {
    await Deno.permissions.query({ name: "net", host: ":" });
  }, URIError);
});

Deno.test(async function permissionQueryReturnsEventTarget() {
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

Deno.test(async function permissionQueryForReadReturnsSameStatus() {
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

Deno.test(function permissionsIllegalConstructor() {
  assertThrows(() => new Deno.Permissions(), TypeError, "Illegal constructor.");
  assertEquals(Deno.Permissions.length, 0);
});

Deno.test(function permissionStatusIllegalConstructor() {
  assertThrows(
    () => new Deno.PermissionStatus(),
    TypeError,
    "Illegal constructor.",
  );
  assertEquals(Deno.PermissionStatus.length, 0);
});

Deno.test(async function permissionURL() {
  await Deno.permissions.query({
    name: "read",
    path: new URL(".", import.meta.url),
  });
  await Deno.permissions.query({
    name: "write",
    path: new URL(".", import.meta.url),
  });
  await Deno.permissions.query({
    name: "run",
    command: new URL(".", import.meta.url),
  });
});
