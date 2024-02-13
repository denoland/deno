// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { includesValue } from "./includes_value.ts";
import { assert, assertEquals } from "../assert/mod.ts";

Deno.test("[collections/includesValue] Example", () => {
  const input = {
    first: 33,
    second: 34,
  };
  const actual = includesValue(input, 34);
  assert(actual);
});

Deno.test("[collections/includesValue] No mutation", () => {
  const input = {
    first: 33,
    second: 34,
  };

  includesValue(input, 34);

  assertEquals(input, {
    first: 33,
    second: 34,
  });
});

Deno.test("[collections/includesValue] Empty input returns false", () => {
  const input = {};

  const actual = includesValue(input, 44);

  assert(!actual);
});

Deno.test("[collections/includesValue] Returns false when it doesn't include the value", () => {
  const input = {
    first: 33,
    second: 34,
  };

  const actual = includesValue(input, 45);

  assert(!actual);
});

Deno.test("[collections/includesValue] Non-enumerable properties", () => {
  // FAIL is expected, TODO: Figure out how to make it work on
  const input = {};

  Object.defineProperty(input, "nep", {
    enumerable: false,
    value: 42,
  });

  Object.defineProperty(input, "neptwo", {
    enumerable: false,
    value: "hello",
  });

  Object.defineProperty(input, "nepthree", {
    enumerable: false,
    value: true,
  });

  const actual1 = includesValue(input, 42);
  const actual2 = includesValue(input, "hello");
  const actual3 = includesValue(input, true);

  assert(!actual1);
  assert(!actual2);
  assert(!actual3);
});

Deno.test("[collections/includesValue] Non-primitive values", () => {
  const input = {
    first: {},
  };

  const actual = includesValue(input, {});

  assert(!actual);
});

Deno.test("[collections/includesValue] Same behaviour as naive impl", () => {
  const input = {
    first: 42,
  };

  const includesValueResult = includesValue(input, 42);
  const naiveImplResult = Object.values(input).includes(42);

  assertEquals(includesValueResult, naiveImplResult);
});

Deno.test("[collections/includesValue] Works with NaN", () => {
  const input = {
    first: NaN,
  };

  const actual = includesValue(input, NaN);

  assert(actual);
});

Deno.test("[collections/includesValue] prevent enumerable prototype check", () => {
  class Foo {}
  // @ts-ignore: for test
  Foo.prototype.a = "hello";
  const input = new Foo() as Record<string, string>;

  const actual = includesValue(input, "hello");

  assert(!actual);
});
