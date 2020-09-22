// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertThrows, assertThrowsAsync } from "./test_util.ts";

unitTest(async function permissionInvalidName(): Promise<void> {
  await assertThrowsAsync(async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
});
