// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function readFileSyncSuccess() {
  const data = deno.readFileSync("package.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEqual(pkg.name, "deno");
});

test(function readFileSyncNotFound() {
  let caughtError = false;
  let data;
  try {
    data = deno.readFileSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assert(data === undefined);
});

test(async function readFileSuccess() {
  const data = await deno.readFile("package.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEqual(pkg.name, "deno");
});
