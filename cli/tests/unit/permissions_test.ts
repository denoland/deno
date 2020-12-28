// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";

unitTest(async function permissionInvalidName(): Promise<void> {
  await assertThrowsAsync(async () => {
    // deno-lint-ignore no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  }, Error);
});

unitTest(async function permissionNetInvalidUrl(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.permissions.query({ name: "net", url: ":" });
  }, URIError);
});

unitTest(function permissionsIllegalConstructor() {
  assertThrows(() => new Deno.Permissions(), TypeError, "Illegal constructor.");
});

unitTest(function permissionStatusIllegalConstructor() {
  assertThrows(
    () => new Deno.PermissionStatus(),
    TypeError,
    "Illegal constructor.",
  );
  assertEquals(Deno.PermissionStatus.length, 0);
});
