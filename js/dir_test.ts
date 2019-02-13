// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";

test(function dirCwdNotNull() {
  assert(Deno.cwd() != null);
});

testPerm({ write: true }, function dirCwdChdirSuccess() {
  const initialdir = Deno.cwd();
  const path = Deno.makeTempDirSync();
  Deno.chdir(path);
  const current = Deno.cwd();
  if (Deno.platform.os === "mac") {
    assertEqual(current, "/private" + path);
  } else {
    assertEqual(current, path);
  }
  Deno.chdir(initialdir);
});

testPerm({ write: true }, function dirCwdError() {
  // excluding windows since it throws resource busy, while removeSync
  if (["linux", "mac"].includes(Deno.platform.os)) {
    const initialdir = Deno.cwd();
    const path = Deno.makeTempDirSync();
    Deno.chdir(path);
    Deno.removeSync(path);
    try {
      Deno.cwd();
      throw Error("current directory removed, should throw error");
    } catch (err) {
      if (err instanceof Deno.DenoError) {
        console.log(err.name === "NotFound");
      } else {
        throw Error("raised different exception");
      }
    }
    Deno.chdir(initialdir);
  }
});

testPerm({ write: true }, function dirChdirError() {
  const path = Deno.makeTempDirSync() + "test";
  try {
    Deno.chdir(path);
    throw Error("directory not available, should throw error");
  } catch (err) {
    if (err instanceof Deno.DenoError) {
      console.log(err.name === "NotFound");
    } else {
      throw Error("raised different exception");
    }
  }
});
