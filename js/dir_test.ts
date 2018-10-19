// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function dirCwdNotNull() {
  assert(deno.cwd() != null);
});

testPerm({ write: true }, function dirCwdChdirSuccess() {
  const initialdir = deno.cwd();
  const path = deno.makeTempDirSync();
  deno.chdir(path);
  const current = deno.cwd();
  if (deno.platform.os === "mac") {
    assertEqual(current, "/private" + path);
  } else {
    assertEqual(current, path);
  }
  deno.chdir(initialdir);
});

testPerm({ write: true }, function dirCwdError() {
  // excluding windows since it throws resource busy, while removeSync
  if (["linux", "mac"].includes(deno.platform.os)) {
    const initialdir = deno.cwd();
    const path = deno.makeTempDirSync();
    deno.chdir(path);
    deno.removeSync(path);
    try {
      deno.cwd();
      throw Error("current directory removed, should throw error");
    } catch (err) {
      if (err instanceof deno.DenoError) {
        console.log(err.name === "NotFound");
      } else {
        throw Error("raised different exception");
      }
    }
    deno.chdir(initialdir);
  }
});

testPerm({ write: true }, function dirChdirError() {
  const path = deno.makeTempDirSync() + "test";
  try {
    deno.chdir(path);
    throw Error("directory not available, should throw error");
  } catch (err) {
    if (err instanceof deno.DenoError) {
      console.log(err.name === "NotFound");
    } else {
      throw Error("raised different exception");
    }
  }
});
