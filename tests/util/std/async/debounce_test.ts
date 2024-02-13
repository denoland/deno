// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStrictEquals } from "../assert/mod.ts";
import { debounce, DebouncedFunction } from "./debounce.ts";
import { delay } from "./delay.ts";

Deno.test("[async] debounce: called", async function () {
  let called = 0;
  const d = debounce(() => called++, 100);
  d();
  d();
  d();
  assertEquals(called, 0);
  assertEquals(d.pending, true);
  await delay(200);
  assertEquals(called, 1);
  assertEquals(d.pending, false);
});

Deno.test("[async] debounce: canceled", async function () {
  let called = 0;
  const d = debounce(() => called++, 100);
  d();
  d();
  d();
  assertEquals(called, 0);
  assertEquals(d.pending, true);
  d.clear();
  await delay(200);
  assertEquals(called, 0);
  assertEquals(d.pending, false);
});

Deno.test("[async] debounce: flushed", function () {
  let called = 0;
  const d = debounce(() => called++, 100);
  d();
  d();
  d();
  assertEquals(called, 0);
  assertEquals(d.pending, true);
  d.flush();
  assertEquals(called, 1);
  assertEquals(d.pending, false);
});

Deno.test("[async] debounce: with params & context", async function () {
  const params: Array<string | number> = [];
  const d: DebouncedFunction<[string, number]> = debounce(
    function (param1: string, param2: number) {
      assertEquals(d.pending, false);
      params.push(param1);
      params.push(param2);
      assertStrictEquals(d, this);
    },
    100,
  );
  // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'string'.
  d(1, 1);
  d("foo", 1);
  d("bar", 1);
  d("baz", 1);
  assertEquals(params.length, 0);
  assertEquals(d.pending, true);
  await delay(200);
  assertEquals(params, ["baz", 1]);
  assertEquals(d.pending, false);
});

Deno.test("[async] debounce: with types", async function () {
  const params: Array<string> = [];
  const fn = (param: string) => params.push(param);
  const d: DebouncedFunction<[string]> = debounce(fn, 100);
  // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'string'.
  d(1);
  d("foo");
  assertEquals(params.length, 0);
  assertEquals(d.pending, true);
  await delay(200);
  assertEquals(params, ["foo"]);
  assertEquals(d.pending, false);
});
