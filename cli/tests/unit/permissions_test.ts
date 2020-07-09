// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { unitTest, assert, assertThrowsAsync } from "./test_util.ts";

unitTest(async function permissionInvalidName(): Promise<void> {
  await assertThrowsAsync(async () => {
    // @ts-expect-error name should not accept "foo"
    await navigator.permissions.query({ name: "foo" });
  }, TypeError);
});

unitTest(async function permissionNetInvalidUrl(): Promise<void> {
  await assertThrowsAsync(async () => {
    await navigator.permissions.query({ name: "net", url: ":" });
  }, URIError);
});

unitTest(async function permissionQueryReturnsEventTarget(): Promise<void> {
  const status = await navigator.permissions.query({ name: "hrtime" });
  assert(["granted", "denied", "prompt"].includes(status.state));
  let called = false;
  status.addEventListener("change", () => {
    called = true;
  });
  status.dispatchEvent(new Event("change"));
  assert(called);
  assert(status === (await navigator.permissions.query({ name: "hrtime" })));
});

unitTest(async function permissionQueryForReadReturnsSameStatus() {
  const status1 = await navigator.permissions.query({
    name: "read",
    path: ".",
  });
  const status2 = await navigator.permissions.query({
    name: "read",
    path: ".",
  });
  assert(status1 === status2);
});
