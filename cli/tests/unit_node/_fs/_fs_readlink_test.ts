// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assertEquals,
  assertThrows,
} from "../../../../test_util/std/testing/asserts.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  mkdirSync,
  mkdtempSync,
  readlink,
  readlinkSync,
  symlinkSync,
} from "node:fs";
import { pathToAbsoluteFileUrl } from "../../unit/test_util.ts";

Deno.test(
  "[node/fs readLink] read link match target path",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    mkdirSync(target);
    symlinkSync(target, symlink);
    readlink(symlink, { encoding: "utf-8" }, (err, link) => {
      assertEquals(err, null);
      assertEquals(link, target);
    });
  },
);

Deno.test(
  "[node/fs readLink] read link match target absolute file url",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    mkdirSync(target);
    symlinkSync(target, symlink);
    readlink(pathToAbsoluteFileUrl(symlink), {
      encoding: "utf-8",
    }, (err, symlink) => {
      assertEquals(err, null);
      assertEquals(target, symlink);
    });
  },
);

Deno.test(
  "[node/fs readLink] read link can not found file",
  () => {
    readlink("bad_filename", { encoding: "utf-8" }, (err, link) => {
      assertEquals(link, undefined);
      assertEquals(
        err?.message,
        "No such file or directory (os error 2), readlink 'bad_filename'",
      );
    });
  },
);

Deno.test(
  "[node/fs readLinkSync] read link match target path",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    mkdirSync(target);
    symlinkSync(target, symlink);
    const targetPath = readlinkSync(symlink, { encoding: "utf-8" });
    assertEquals(targetPath, target);
  },
);

Deno.test(
  "[node/fs readLinkSync] read link match target absolute file url",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    mkdirSync(target);
    symlinkSync(target, symlink);
    const targetPath = readlinkSync(pathToAbsoluteFileUrl(symlink), {
      encoding: "utf-8",
    });
    assertEquals(targetPath, target);
  },
);

Deno.test(
  "[node/fs readLinkSync] read link throws not found file",
  () => {
    assertThrows(
      () => readlinkSync("bad_filename", { encoding: "utf-8" }),
      Deno.errors.NotFound,
      `readlink 'bad_filename'`,
    );
  },
);

Deno.test(
  "[node/fs readLinkSync] read link throws when encoding is not yet implemented",
  () => {
    assertThrows(
      () => readlinkSync("bad_filename", { encoding: "utf16le" }),
      'The value "utf16le" is invalid for option "encoding"',
    );
  },
);
