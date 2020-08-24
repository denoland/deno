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

unitTest(function permissionSyncInvalidName(): void {
  assertThrows(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    Deno.permissions.querySync({ name: "foo" as any });
  }, Error);
});

unitTest(function permissionSyncNetInvalidUrl(): void {
  assertThrows(() => {
    Deno.permissions.querySync({ name: "net", url: ":" });
  }, URIError);
});
