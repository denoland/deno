// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { green, red, stripColor } from "../fmt/colors.ts";
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { format } from "./_format.ts";

Deno.test("assert diff formatting (strings)", () => {
  assertThrows(
    () => {
      assertEquals([..."abcd"].join("\n"), [..."abxde"].join("\n"));
    },
    Error,
    `
    a\\n
    b\\n
${green("+   x")}\\n
${green("+   d")}\\n
${green("+   e")}
${red("-   c")}\\n
${red("-   d")}
`,
  );
});

// Check that the diff formatter overrides some default behaviours of
// `Deno.inspect()` which are problematic for diffing.
Deno.test("assert diff formatting", () => {
  // Wraps objects into multiple lines even when they are small. Prints trailing
  // commas.
  assertEquals(
    stripColor(format({ a: 1, b: 2 })),
    `{
  a: 1,
  b: 2,
}`,
  );

  // Wraps Object with getters
  assertEquals(
    format(Object.defineProperty({}, "a", {
      enumerable: true,
      get() {
        return 1;
      },
    })),
    `{
  a: [Getter: 1],
}`,
  );

  // Same for nested small objects.
  assertEquals(
    stripColor(format([{ x: { a: 1, b: 2 }, y: ["a", "b"] }])),
    `[
  {
    x: {
      a: 1,
      b: 2,
    },
    y: [
      "a",
      "b",
    ],
  },
]`,
  );

  // Grouping is disabled.
  assertEquals(
    stripColor(format(["i", "i", "i", "i", "i", "i", "i"])),
    `[
  "i",
  "i",
  "i",
  "i",
  "i",
  "i",
  "i",
]`,
  );
});
