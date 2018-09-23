// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, async function readDirSuccess() {
  const dirName = deno.makeTempDirSync();
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  deno.writeFileSync(`${dirName}/test.txt`, data, 0o666);
  deno.writeFileSync(`${dirName}/test.rs`, data, 0o666);

  const entries = deno.readDirSync(dirName);
  assertEqual(entries.length, 2);

  for (const entry of entries) {
    assert(entry.isFile());
    assert(entry.name === "test.txt" || entry.name === "test.rs");
    assert(
      entry.path === `${dirName}/test.txt` ||
      entry.path === `${dirName}/test.rs`
    );
  }
});

test(async function readDirSyncNotADir() {
  let caughtError = false;

  try {
    const src = deno.readDirSync("Cargo.toml");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.Other);
  }

  assert(caughtError);
});

test(async function readDirSyncNotFound() {
  let caughtError = false;

  try {
    const src = deno.readDirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
  }

  assert(caughtError);
});
