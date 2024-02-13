// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  bold,
  gray,
  green,
  red,
  stripAnsiCode,
  yellow,
} from "../fmt/colors.ts";
import { assertThrows } from "../assert/assert_throws.ts";
import { AssertionError } from "../assert/assertion_error.ts";
import { expect } from "./expect.ts";

const createHeader = (): string[] => [
  "",
  "",
  `    ${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${
    green(
      bold("Expected"),
    )
  }`,
  "",
  "",
];

const added: (s: string) => string = (s: string): string =>
  green(bold(stripAnsiCode(s)));
const removed: (s: string) => string = (s: string): string =>
  red(bold(stripAnsiCode(s)));

Deno.test({
  name: "pass case",
  fn() {
    expect({ a: 10 }).toEqual({ a: 10 });
    expect(true).toEqual(true);
    expect(10).toEqual(10);
    expect("abc").toEqual("abc");
    expect({ a: 10, b: { c: "1" } }).toEqual({ a: 10, b: { c: "1" } });
    expect(new Date("invalid")).toEqual(new Date("invalid"));
  },
});

Deno.test({
  name: "failed with number",
  fn() {
    assertThrows(
      () => expect(1).toEqual(2),
      AssertionError,
      [
        "Values are not equal.",
        ...createHeader(),
        removed(`-   ${yellow("1")}`),
        added(`+   ${yellow("2")}`),
        "",
      ].join("\n"),
    );
  },
});

Deno.test({
  name: "failed with number vs string",
  fn() {
    assertThrows(
      () => expect(1).toEqual("1"),
      AssertionError,
      [
        "Values are not equal.",
        ...createHeader(),
        removed(`-   ${yellow("1")}`),
        added(`+   "1"`),
      ].join("\n"),
    );
  },
});

Deno.test({
  name: "failed with array",
  fn() {
    assertThrows(
      () => expect([1, "2", 3]).toEqual(["1", "2", 3]),
      AssertionError,
      `
    [
-     1,
+     "1",
      "2",
      3,
    ]`,
    );
  },
});

Deno.test({
  name: "failed with object",
  fn() {
    assertThrows(
      () => expect({ a: 1, b: "2", c: 3 }).toEqual({ a: 1, b: 2, c: [3] }),
      AssertionError,
      `
    {
      a: 1,
+     b: 2,
+     c: [
+       3,
+     ],
-     b: "2",
-     c: 3,
    }`,
    );
  },
});

Deno.test({
  name: "failed with date",
  fn() {
    assertThrows(
      () =>
        expect(new Date(2019, 0, 3, 4, 20, 1, 10)).toEqual(
          new Date(2019, 0, 3, 4, 20, 1, 20),
        ),
      AssertionError,
      [
        "Values are not equal.",
        ...createHeader(),
        removed(`-   ${new Date(2019, 0, 3, 4, 20, 1, 10).toISOString()}`),
        added(`+   ${new Date(2019, 0, 3, 4, 20, 1, 20).toISOString()}`),
        "",
      ].join("\n"),
    );
    assertThrows(
      () =>
        expect(new Date("invalid")).toEqual(new Date(2019, 0, 3, 4, 20, 1, 20)),
      AssertionError,
      [
        "Values are not equal.",
        ...createHeader(),
        removed(`-   ${new Date("invalid")}`),
        added(`+   ${new Date(2019, 0, 3, 4, 20, 1, 20).toISOString()}`),
        "",
      ].join("\n"),
    );
  },
});

Deno.test({
  name: "failed with custom msg",
  fn() {
    assertThrows(
      () => expect(1, "CUSTOM MESSAGE").toEqual(2),
      AssertionError,
      [
        "Values are not equal: CUSTOM MESSAGE",
        ...createHeader(),
        removed(`-   ${yellow("1")}`),
        added(`+   ${yellow("2")}`),
        "",
      ].join("\n"),
    );
  },
});

Deno.test(
  "expect().toEqual compares objects structurally if one object's constructor is undefined and the other is Object",
  () => {
    const a = Object.create(null);
    a.prop = "test";
    const b = {
      prop: "test",
    };

    expect(a).toEqual(b);
    expect(b).toEqual(a);
  },
);

Deno.test("expect().toEqual diff for differently ordered objects", () => {
  assertThrows(
    () => {
      expect({
        aaaaaaaaaaaaaaaaaaaaaaaa: 0,
        bbbbbbbbbbbbbbbbbbbbbbbb: 0,
        ccccccccccccccccccccccc: 0,
      }).toEqual(
        {
          ccccccccccccccccccccccc: 1,
          aaaaaaaaaaaaaaaaaaaaaaaa: 0,
          bbbbbbbbbbbbbbbbbbbbbbbb: 0,
        },
      );
    },
    AssertionError,
    `
    {
      aaaaaaaaaaaaaaaaaaaaaaaa: 0,
      bbbbbbbbbbbbbbbbbbbbbbbb: 0,
-     ccccccccccccccccccccccc: 0,
+     ccccccccccccccccccccccc: 1,
    }`,
  );
});

Deno.test("expect().toEqual same Set with object keys", () => {
  const data = [
    {
      id: "_1p7ZED73OF98VbT1SzSkjn",
      type: { id: "_ETGENUS" },
      name: "Thuja",
      friendlyId: "g-thuja",
    },
    {
      id: "_567qzghxZmeQ9pw3q09bd3",
      type: { id: "_ETGENUS" },
      name: "Pinus",
      friendlyId: "g-pinus",
    },
  ];
  expect(data).toEqual(data);
  expect(new Set(data)).toEqual(new Set(data));
});
