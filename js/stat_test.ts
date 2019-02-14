// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

const isWindows = Deno.platform.os === "win";

async function success(
  func: ((string) => Deno.FileInfo) | ((string) => Promise<Deno.FileInfo>)
): Promise<void> {
  const packageInfo = await func("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isDirectory());
  assert(!packageInfo.isSymlink());
  assertEquals(packageInfo.name, "package.json");
  assertEquals(packageInfo.path, "package.json");

  const testingInfo = await func("testing");
  if (func.name.includes("lstat")) {
    assert(testingInfo.isSymlink());
    assert(!testingInfo.isFile());
  } else {
    assert(!testingInfo.isSymlink());
    assert(testingInfo.isDirectory());
  }
  assertEquals(testingInfo.name, "testing");
  assertEquals(testingInfo.path, "testing");

  const srcInfo = await func("src");
  assert(!srcInfo.isFile());
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
  assertEquals(srcInfo.name, "src");
  assertEquals(srcInfo.path, "src");

  const jsOs = isWindows ? "js\\os.ts" : "js/os.ts";
  const jsOsInfo = await func(jsOs);
  assert(jsOsInfo.isFile());
  assert(!jsOsInfo.isDirectory());
  assert(!jsOsInfo.isSymlink());
  assertEquals(jsOsInfo.name, "os.ts");
  assertEquals(jsOsInfo.path, jsOs);
}

async function permFail(
  func: ((string) => Deno.FileInfo) | ((string) => Promise<Deno.FileInfo>)
): Promise<void> {
  let caughtError = false;
  try {
    await func("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
}

async function notFound(
  func: ((string) => Deno.FileInfo) | ((string) => Promise<Deno.FileInfo>)
): Promise<void> {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await func("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
}

// TODO Add tests for modified, accessed, and created fields once there is a way
// to create temp files.

for (const func of [Deno.stat, Deno.statSync, Deno.lstat, Deno.lstatSync]) {
  testPerm(
    { read: true },
    {
      fn: async () => {
        await success(func);
      },
      name: func.name + "Success"
    }
  );
  testPerm(
    { read: false },
    {
      fn: async () => {
        await permFail(func);
      },
      name: func.name + "Perm"
    }
  );
  testPerm(
    { read: true },
    {
      fn: async () => {
        await notFound(func);
      },
      name: func.name + "NotFound"
    }
  );
}
