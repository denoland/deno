// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import { deferred } from "./deferred.ts";

Deno.test("[async] deferred: resolve", async function (): Promise<void> {
  const d = deferred<string>();
  d.resolve("🦕");
  assertEquals(await d, "🦕");
});

Deno.test("[async] deferred: reject", async function (): Promise<void> {
  const d = deferred<number>();
  d.reject(new Error("A deno error 🦕"));
  await assertThrowsAsync(async () => {
    await d;
  });
});
