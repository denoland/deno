// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

unitTest({ perms: { read: true } }, function dirCwdNotNull(): void {
  assert(Deno.cwd() != null);
});

unitTest(
  { perms: { read: true, write: true } },
  function dirCwdChdirSuccess(): void {
    const initialdir = Deno.cwd();
    const path = Deno.makeTempDirSync();
    Deno.chdir(path);
    const current = Deno.cwd();
    if (Deno.build.os === "darwin") {
      assertEquals(current, "/private" + path);
    } else {
      assertEquals(current, path);
    }
    Deno.chdir(initialdir);
  },
);

unitTest({ perms: { read: true, write: true } }, function dirCwdError(): void {
  // excluding windows since it throws resource busy, while removeSync
  if (["linux", "darwin"].includes(Deno.build.os)) {
    const initialdir = Deno.cwd();
    const path = Deno.makeTempDirSync();
    Deno.chdir(path);
    Deno.removeSync(path);
    try {
      assertThrows(() => {
        Deno.cwd();
      }, Deno.errors.NotFound);
    } finally {
      Deno.chdir(initialdir);
    }
  }
});

unitTest({ perms: { read: false } }, function dirCwdPermError(): void {
  assertThrows(
    () => {
      Deno.cwd();
    },
    Deno.errors.PermissionDenied,
    "Access to read <CWD>, run again with read permission",
  );
});

unitTest(
  { perms: { read: true, write: true } },
  function dirChdirError(): void {
    const path = Deno.makeTempDirSync() + "test";
    assertThrows(() => {
      Deno.chdir(path);
    }, Deno.errors.NotFound);
  },
);
