// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// This test is executed as part of integration_test.go
// But it can also be run manually:
//  ./deno tests.ts
// There must also be a static file http server running on localhost:4545
// serving the deno project directory. Try this:
//   http-server -p 4545 --cors .
import { test, assert, assertEqual } from "./testing/testing.ts";
import { readFileSync, writeFileSync, process } from "deno";

test(async function tests_test() {
  assert(true);
});

test(async function tests_fetch() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});

test(function tests_console_assert() {
  console.assert(true);

  let hasThrown = false;
  try {
    console.assert(false);
  } catch {
    hasThrown = true;
  }
  assertEqual(hasThrown, true);
});

test(async function tests_readFileSync() {
  const data = readFileSync("package.json");
  if (!data.byteLength) {
    throw Error(
      `Expected positive value for data.byteLength ${data.byteLength}`
    );
  }
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEqual(pkg.name, "deno");
});

test(async function tests_writeFileSync() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const proc = process();
  // TODO: not support windows yet
  const fn = proc.tmpDir.endsWith("/")
      ? proc.tmpDir + "text.txt"
      : proc.tmpDir +"/text.txt";
  writeFileSync(fn, data, 0o666);
  const dataRead = readFileSync(fn);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

test(async function tests_process() {
  const {platform, cwd, tmpDir} = process();
  assert(
      ["darwin", "freebsd", "linux"].includes(platform),
      "platform not exists: " + platform);
  assert(cwd.length > 0, "get cwd failed");
  assert(tmpDir.length >0, "get tempdir failed");
});
