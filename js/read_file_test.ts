// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ read: true }, function readFileSyncSuccess() {
  const data = Deno.readFileSync("package.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

testPerm({ read: false }, function readFileSyncPerm() {
  let caughtError = false;
  try {
    Deno.readFileSync("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, function readFileSyncNotFound() {
  let caughtError = false;
  let data;
  try {
    data = Deno.readFileSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assert(data === undefined);
});

testPerm({ read: true }, async function readFileSuccess() {
  const data = await Deno.readFile("package.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

testPerm({ read: false }, async function readFilePerm() {
  let caughtError = false;
  try {
    await Deno.readFile("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});
