// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { diff, diffstr, DiffType } from "./_diff.ts";
import { assertEquals } from "./assert_equals.ts";

Deno.test({
  name: "empty",
  fn() {
    assertEquals(diff([], []), []);
  },
});

Deno.test({
  name: '"a" vs "b"',
  fn() {
    assertEquals(diff(["a"], ["b"]), [
      { type: DiffType.removed, value: "a" },
      { type: DiffType.added, value: "b" },
    ]);
  },
});

Deno.test({
  name: '"a" vs "a"',
  fn() {
    assertEquals(diff(["a"], ["a"]), [{ type: DiffType.common, value: "a" }]);
  },
});

Deno.test({
  name: '"a" vs ""',
  fn() {
    assertEquals(diff(["a"], []), [{ type: DiffType.removed, value: "a" }]);
  },
});

Deno.test({
  name: '"" vs "a"',
  fn() {
    assertEquals(diff([], ["a"]), [{ type: DiffType.added, value: "a" }]);
  },
});

Deno.test({
  name: '"a" vs "a, b"',
  fn() {
    assertEquals(diff(["a"], ["a", "b"]), [
      { type: DiffType.common, value: "a" },
      { type: DiffType.added, value: "b" },
    ]);
  },
});

Deno.test({
  name: '"strength" vs "string"',
  fn() {
    assertEquals(diff(Array.from("strength"), Array.from("string")), [
      { type: DiffType.common, value: "s" },
      { type: DiffType.common, value: "t" },
      { type: DiffType.common, value: "r" },
      { type: DiffType.removed, value: "e" },
      { type: DiffType.added, value: "i" },
      { type: DiffType.common, value: "n" },
      { type: DiffType.common, value: "g" },
      { type: DiffType.removed, value: "t" },
      { type: DiffType.removed, value: "h" },
    ]);
  },
});

Deno.test({
  name: '"strength" vs ""',
  fn() {
    assertEquals(diff(Array.from("strength"), Array.from("")), [
      { type: DiffType.removed, value: "s" },
      { type: DiffType.removed, value: "t" },
      { type: DiffType.removed, value: "r" },
      { type: DiffType.removed, value: "e" },
      { type: DiffType.removed, value: "n" },
      { type: DiffType.removed, value: "g" },
      { type: DiffType.removed, value: "t" },
      { type: DiffType.removed, value: "h" },
    ]);
  },
});

Deno.test({
  name: '"" vs "strength"',
  fn() {
    assertEquals(diff(Array.from(""), Array.from("strength")), [
      { type: DiffType.added, value: "s" },
      { type: DiffType.added, value: "t" },
      { type: DiffType.added, value: "r" },
      { type: DiffType.added, value: "e" },
      { type: DiffType.added, value: "n" },
      { type: DiffType.added, value: "g" },
      { type: DiffType.added, value: "t" },
      { type: DiffType.added, value: "h" },
    ]);
  },
});

Deno.test({
  name: '"abc", "c" vs "abc", "bcd", "c"',
  fn() {
    assertEquals(diff(["abc", "c"], ["abc", "bcd", "c"]), [
      { type: DiffType.common, value: "abc" },
      { type: DiffType.added, value: "bcd" },
      { type: DiffType.common, value: "c" },
    ]);
  },
});

Deno.test({
  name: '"a b c d" vs "a b x d e" (diffstr)',
  fn() {
    const diffResult = diffstr(
      [..."abcd"].join("\n"),
      [..."abxde"].join("\n"),
    );
    assertEquals(diffResult, [
      { type: DiffType.common, value: "a\\n\n" },
      { type: DiffType.common, value: "b\\n\n" },
      {
        type: DiffType.added,
        value: "x\\n\n",
        details: [
          { type: DiffType.added, value: "x" },
          { type: DiffType.common, value: "\\" },
          { type: DiffType.common, value: "n" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.added,
        value: "d\\n\n",
        details: [
          { type: DiffType.common, value: "d" },
          { type: DiffType.added, value: "\\" },
          { type: DiffType.added, value: "n" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      { type: DiffType.added, value: "e\n" },
      {
        type: DiffType.removed,
        value: "c\\n\n",
        details: [
          { type: DiffType.removed, value: "c" },
          { type: DiffType.common, value: "\\" },
          { type: DiffType.common, value: "n" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.removed,
        value: "d\n",
        details: [
          { type: DiffType.common, value: "d" },
          { type: DiffType.common, value: "\n" },
        ],
      },
    ]);
  },
});

Deno.test({
  name: `"3.14" vs "2.71" (diffstr)`,
  fn() {
    const diffResult = diffstr("3.14", "2.71");
    assertEquals(diffResult, [
      {
        type: DiffType.removed,
        value: "3.14\n",
        details: [
          {
            type: DiffType.removed,
            value: "3",
          },
          {
            type: DiffType.common,
            value: ".",
          },
          {
            type: DiffType.removed,
            value: "14",
          },
          {
            type: DiffType.common,
            value: "\n",
          },
        ],
      },
      {
        type: DiffType.added,
        value: "2.71\n",
        details: [
          {
            type: DiffType.added,
            value: "2",
          },
          {
            type: DiffType.common,
            value: ".",
          },
          {
            type: DiffType.added,
            value: "71",
          },
          {
            type: DiffType.common,
            value: "\n",
          },
        ],
      },
    ]);
  },
});

Deno.test({
  name: `single line "a b" vs "c d" (diffstr)`,
  fn() {
    const diffResult = diffstr("a b", "c d");
    assertEquals(diffResult, [
      {
        type: DiffType.removed,
        value: "a b\n",
        details: [
          { type: DiffType.removed, value: "a" },
          { type: DiffType.removed, value: " " },
          { type: DiffType.removed, value: "b" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.added,
        value: "c d\n",
        details: [
          { type: DiffType.added, value: "c" },
          { type: DiffType.added, value: " " },
          { type: DiffType.added, value: "d" },
          { type: DiffType.common, value: "\n" },
        ],
      },
    ]);
  },
});

Deno.test({
  name: `single line, different word length "a bc" vs "cd e" (diffstr)`,
  fn() {
    const diffResult = diffstr("a bc", "cd e");
    assertEquals(diffResult, [
      {
        type: DiffType.removed,
        value: "a bc\n",
        details: [
          { type: DiffType.removed, value: "a" },
          { type: DiffType.removed, value: " " },
          { type: DiffType.removed, value: "bc" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.added,
        value: "cd e\n",
        details: [
          { type: DiffType.added, value: "cd" },
          { type: DiffType.added, value: " " },
          { type: DiffType.added, value: "e" },
          { type: DiffType.common, value: "\n" },
        ],
      },
    ]);
  },
});

Deno.test({
  name: `"\\b\\f\\r\\t\\v\\n" vs "\\r\\n" (diffstr)`,
  fn() {
    const diffResult = diffstr("\b\f\r\t\v\n", "\r\n");
    assertEquals(diffResult, [
      {
        type: DiffType.removed,
        value: "\\b\\f\\r\\t\\v\\n\n",
        details: [
          { type: DiffType.common, value: "\\" },
          { type: DiffType.removed, value: "b" },
          { type: DiffType.removed, value: "\\" },
          { type: DiffType.removed, value: "f" },
          { type: DiffType.removed, value: "\\" },
          { type: DiffType.common, value: "r" },
          { type: DiffType.common, value: "\\" },
          { type: DiffType.removed, value: "t" },
          { type: DiffType.removed, value: "\\" },
          { type: DiffType.removed, value: "v" },
          { type: DiffType.removed, value: "\\" },
          { type: DiffType.common, value: "n" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.added,
        value: "\\r\\n\r\n",
        details: [
          { type: DiffType.common, value: "\\" },
          { type: DiffType.common, value: "r" },
          { type: DiffType.common, value: "\\" },
          { type: DiffType.common, value: "n" },
          { type: DiffType.added, value: "\r" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      { type: DiffType.common, value: "\n" },
    ]);
  },
});

Deno.test({
  name: "multiline diff with more removed lines",
  fn() {
    const diffResult = diffstr("a\na", "e");
    assertEquals(diffResult, [
      {
        type: DiffType.removed,
        value: "a\\n\n",
      },
      {
        type: DiffType.removed,
        value: "a\n",
        details: [
          { type: DiffType.removed, value: "a" },
          { type: DiffType.common, value: "\n" },
        ],
      },
      {
        type: DiffType.added,
        value: "e\n",
        details: [
          { type: DiffType.added, value: "e" },
          { type: DiffType.common, value: "\n" },
        ],
      },
    ]);
  },
});
