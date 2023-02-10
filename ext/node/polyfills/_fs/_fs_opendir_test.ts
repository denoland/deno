// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertFalse,
  assertInstanceOf,
  assertThrows,
} from "../../testing/asserts.ts";
import { opendir, opendirSync } from "./_fs_opendir.ts";
import { Buffer } from "../buffer.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";

Deno.test("[node/fs] opendir()", async (t) => {
  const path = await Deno.makeTempDir();
  const file = await Deno.makeTempFile();

  await t.step(
    "fails if encoding is invalid",
    () =>
      opendir(
        path,
        { encoding: "invalid-encoding" },
        (err) => assertInstanceOf(err, TypeError),
      ),
  );

  await t.step(
    "fails if bufferSize is invalid",
    () =>
      opendir(
        path,
        { bufferSize: -1 },
        (err) => assertInstanceOf(err, RangeError),
      ),
  );

  await t.step(
    "fails if directory does not exist",
    () =>
      opendir(
        "directory-that-does-not-exist",
        (err) => assertInstanceOf(err, Error),
      ),
  );

  await t.step(
    "fails if not a directory",
    () =>
      opendir(
        file,
        (err) => assertInstanceOf(err, Error),
      ),
  );

  await t.step(
    "passes if path is a string",
    () =>
      opendir(
        path,
        (err, dir) => {
          assertEquals(err, null);
          assert(dir);
        },
      ),
  );

  await t.step(
    "passes if path is a Buffer",
    () =>
      opendir(
        Buffer.from(path),
        (err, dir) => {
          assertFalse(err);
          assert(dir);
        },
      ),
  );

  await t.step(
    "passes if path is a URL",
    () =>
      opendir(
        new URL(`file://` + path),
        (err, dir) => {
          assertFalse(err);
          assert(dir);
        },
      ),
  );

  await t.step("passes if callback isn't called twice", async () => {
    const importUrl = new URL("./_fs_opendir.ts", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { opendir } from ${JSON.stringify(importUrl)}`,
      invocation: `opendir(${JSON.stringify(path)}, `,
    });
  });

  await Deno.remove(path);
  await Deno.remove(file);
});

Deno.test("[node/fs] opendirSync()", async (t) => {
  const path = await Deno.makeTempDir();
  const file = await Deno.makeTempFile();

  await t.step("fails if encoding is invalid", () => {
    assertThrows(
      () => opendirSync(path, { encoding: "invalid-encoding" }),
      TypeError,
    );
  });

  await t.step("fails if bufferSize is invalid", () => {
    assertThrows(
      () => opendirSync(path, { bufferSize: -1 }),
      RangeError,
    );
  });

  await t.step("fails if directory does not exist", () => {
    assertThrows(() => opendirSync("directory-that-does-not-exist"));
  });

  await t.step("fails if not a directory", () => {
    assertThrows(() => opendirSync(file));
  });

  await t.step("passes if path is a string", () => {
    assert(opendirSync(path));
  });

  await t.step("passes if path is a Buffer", () => {
    assert(opendirSync(Buffer.from(path)));
  });

  await t.step("passes if path is a URL", () => {
    assert(opendirSync(new URL(`file://` + path)));
  });

  await Deno.remove(path);
  await Deno.remove(file);
});
