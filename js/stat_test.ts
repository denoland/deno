// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

// TODO Add tests for modified, accessed, and created fields once there is a way
// to create temp files.
test(async function statSyncSuccess() {
  const packageInfo = deno.statSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = deno.statSync("testing");
  assert(testingInfo.isDirectory());
  assert(!testingInfo.isSymlink());

  const srcInfo = deno.statSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function statSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = deno.statSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});

test(async function lstatSyncSuccess() {
  const packageInfo = deno.lstatSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = deno.lstatSync("testing");
  assert(!testingInfo.isDirectory());
  assert(testingInfo.isSymlink());

  const srcInfo = deno.lstatSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function lstatSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = deno.lstatSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});

test(async function statSuccess() {
  const packageInfo = await deno.stat("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = await deno.stat("testing");
  assert(testingInfo.isDirectory());
  assert(!testingInfo.isSymlink());

  const srcInfo = await deno.stat("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function statNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await deno.stat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});

test(async function lstatSuccess() {
  const packageInfo = await deno.lstat("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = await deno.lstat("testing");
  assert(!testingInfo.isDirectory());
  assert(testingInfo.isSymlink());

  const srcInfo = await deno.lstat("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function lstatNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await deno.lstat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});
