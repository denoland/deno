import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function NotNullcwd() {
  assert(deno.cwd() != null);
});

testPerm({ write: true }, function test_cwd_output() {
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  deno.chdir(path);
  assertEqual(deno.cwd(), path);
});

testPerm({ write: true }, function test_cwd_error() {
  const path = deno.makeTempDirSync();
  deno.chdir(path);
  deno.removeSync(path);
  try {
    deno.cwd();
    throw "current directory removed, should throw error";
  } catch (err) {
    if (err instanceof deno.DenoError) {
      console.log(err.name === "NotFound");
    } else {
      throw "raised different exception";
    }
  }
});

testPerm({ write: true }, function test_chdir_throw() {
  const path = deno.makeTempDirSync() + "test";
  try {
    deno.chdir(path);
    throw "directory not available, should throw error";
  } catch (err) {
    if (err instanceof deno.DenoError) {
      console.log(err.name === "NotFound");
    } else {
      throw "raised different exception";
    }
  }
});
