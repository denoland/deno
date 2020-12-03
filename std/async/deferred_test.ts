// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import { Deferred } from "./deferred.ts";

Deno.test("[async] deferred: resolve", async function (): Promise<void> {
  const d = new Deferred<string>();
  d.resolve("🦕");
  assertEquals(await d, "🦕");
});

Deno.test("[async] deferred: reject", async function (): Promise<void> {
  const d = new Deferred<number>();
  d.reject(new Error("A deno error 🦕"));
  await assertThrowsAsync(async () => {
    await d;
  });
});
