// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "@std/assert";
import { parse, stringify } from "node:querystring";

Deno.test({
  name: "stringify",
  fn() {
    assertEquals(
      stringify({
        a: "hello",
        b: 5,
        c: true,
        d: ["foo", "bar"],
      }),
      "a=hello&b=5&c=true&d=foo&d=bar",
    );
  },
});

Deno.test({
  name: "parse",
  fn() {
    assertEquals(parse("a=hello&b=5&c=true&d=foo&d=bar"), {
      a: "hello",
      b: "5",
      c: "true",
      d: ["foo", "bar"],
    });
  },
});

// https://github.com/denoland/deno/issues/21734
Deno.test({
  name: "stringify options no encode",
  fn() {
    assertEquals(
      stringify(
        {
          a: "hello",
          b: 5,
          c: true,
          d: ["foo", "bar"],
        },
        "&",
        "=",
        {},
      ),
      "a=hello&b=5&c=true&d=foo&d=bar",
    );
  },
});
