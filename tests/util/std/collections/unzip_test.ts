// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { unzip } from "./unzip.ts";

function unzipTest<T, U>(
  input: [Array<[T, U]>],
  expected: [Array<T>, Array<U>],
  message?: string,
) {
  const actual = unzip(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/unzip] no mutation",
  fn() {
    const zipped: Array<[number, boolean]> = [
      [1, true],
      [2, false],
      [3, true],
    ];
    unzip(zipped);

    assertEquals(zipped, [
      [1, true],
      [2, false],
      [3, true],
    ]);
  },
});

Deno.test({
  name: "[collections/unzip] empty input",
  fn() {
    unzipTest(
      [[]],
      [[], []],
    );
  },
});

Deno.test({
  name: "[collections/unzip] unzips",
  fn() {
    unzipTest(
      [
        [
          [1, "foo"],
          [4, "bar"],
          [5, "lorem"],
        ],
      ],
      [
        [1, 4, 5],
        ["foo", "bar", "lorem"],
      ],
    );
    unzipTest(
      [
        [
          [true, false],
        ],
      ],
      [
        [true],
        [false],
      ],
    );
    unzipTest(
      [
        [
          [undefined, "foo"],
          [5, null],
          [undefined, "asdf"],
          [null, false],
          [1.2, ""],
        ],
      ],
      [
        [undefined, 5, undefined, null, 1.2],
        ["foo", null, "asdf", false, ""],
      ],
    );
  },
});
