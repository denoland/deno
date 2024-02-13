// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects } from "../assert/mod.ts";
import { abortable } from "./abortable.ts";

Deno.test("[async] abortable (Promise)", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  const result = await abortable(promise, c.signal);
  assertEquals(result, "Hello");
  clearTimeout(t);
});

Deno.test("[async] abortable (Promise) with signal aborted after delay", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  setTimeout(() => c.abort(), 50);
  await assertRejects(
    async () => {
      await abortable(promise, c.signal);
    },
    DOMException,
    "AbortError",
  );
  clearTimeout(t);
});

Deno.test("[async] abortable (Promise) with already aborted signal", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  c.abort();
  await assertRejects(
    async () => {
      await abortable(promise, c.signal);
    },
    DOMException,
    "AbortError",
  );
  clearTimeout(t);
});

Deno.test("[async] abortable (AsyncIterable)", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  const a = async function* () {
    yield "Hello";
    await promise;
    yield "World";
  };
  const items = await Array.fromAsync(abortable(a(), c.signal));
  assertEquals(items, ["Hello", "World"]);
  clearTimeout(t);
});

Deno.test("[async] abortable (AsyncIterable) with signal aborted after delay", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  const a = async function* () {
    yield "Hello";
    await promise;
    yield "World";
  };
  setTimeout(() => c.abort(), 50);
  const items: string[] = [];
  await assertRejects(
    async () => {
      for await (const item of abortable(a(), c.signal)) {
        items.push(item);
      }
    },
    DOMException,
    "AbortError",
  );
  assertEquals(items, ["Hello"]);
  clearTimeout(t);
});

Deno.test("[async] abortable (AsyncIterable) with already aborted signal", async () => {
  const c = new AbortController();
  const { promise, resolve } = Promise.withResolvers<string>();
  const t = setTimeout(() => resolve("Hello"), 100);
  const a = async function* () {
    yield "Hello";
    await promise;
    yield "World";
  };
  c.abort();
  const items: string[] = [];
  await assertRejects(
    async () => {
      for await (const item of abortable(a(), c.signal)) {
        items.push(item);
      }
    },
    DOMException,
    "AbortError",
  );
  assertEquals(items, []);
  clearTimeout(t);
});
