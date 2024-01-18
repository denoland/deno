// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertThrows } from "../../../test_util/std/assert/mod.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { pathToAbsoluteFileUrl } from "../unit/test_util.ts";

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

Deno.test(
  "[node/fs existsSync] path",
  { permissions: { read: true } },
  () => {
    assert(existsSync("cli/tests/testdata/assets/fixture.json"));
  },
);

Deno.test(
  "[node/fs existsSync] url",
  { permissions: { read: true } },
  () => {
    assert(existsSync(
      pathToAbsoluteFileUrl("cli/tests/testdata/assets/fixture.json"),
    ));
  },
);

Deno.test(
  "[node/fs existsSync] no permission",
  { permissions: { read: false } },
  () => {
    assertThrows(() => {
      existsSync("cli/tests/testdata/assets/fixture.json");
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  "[node/fs existsSync] not exists",
  { permissions: { read: true } },
  () => {
    assert(!existsSync("bad_filename"));
  },
);
