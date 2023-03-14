// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertThrows,
} from "../../../test_util/std/testing/asserts.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";

Deno.test(
  "[node/fs writeFileSync] write file without option",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    writeFileSync(filename, data);
    const dataRead = readFileSync(filename, "utf8");

    assert(dataRead === "Hello");
  },
);

Deno.test(
  "[node/fs writeFileSync] write file with option ASCII",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    writeFileSync(filename, data, { encoding: "ascii" });
    const dataRead = readFileSync(filename, "utf8");

    assert(dataRead === "Hello");
  },
);

Deno.test(
  "[node/fs writeFileSync] write file throws error when encoding is not implemented",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    assertThrows(
      () => writeFileSync(filename, data, { encoding: "utf16le" }),
      'The value "utf16le" is invalid for option "encoding"',
    );
  },
);
