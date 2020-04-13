// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { assertEquals, assert } from "../testing/asserts.ts";
import { deepAssign } from "./deep_assign.ts";

test(function deepAssignTest(): void {
  const date = new Date("1979-05-27T07:32:00Z");
  const reg = RegExp(/DENOWOWO/);
  const obj1 = { deno: { bar: { deno: ["is", "not", "node"] } } };
  const obj2 = { foo: { deno: date } };
  const obj3 = { foo: { bar: "deno" }, reg: reg };
  const actual = deepAssign(obj1, obj2, obj3);
  const expected = {
    foo: {
      deno: new Date("1979-05-27T07:32:00Z"),
      bar: "deno",
    },
    deno: { bar: { deno: ["is", "not", "node"] } },
    reg: RegExp(/DENOWOWO/),
  };
  assert(date !== expected.foo.deno);
  assert(reg !== expected.reg);
  assertEquals(actual, expected);
});
