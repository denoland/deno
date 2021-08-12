// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

unitTest({ perms: { read: true } }, function dirCwdNotNull() {
  assert(Deno.cwd() != null);
});

unitTest(
  { perms: { read: true, write: true } },
  function dirCwdChdirSuccess() {
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

unitTest({ perms: { read: true, write: true } }, function dirCwdError() {
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

unitTest({ perms: { read: false } }, function dirCwdPermError() {
  assertThrows(
    () => {
      Deno.cwd();
    },
    Deno.errors.PermissionDenied,
    "Requires read access to <CWD>, run again with the --allow-read flag",
  );
});

unitTest(
  { perms: { read: true, write: true } },
  function dirChdirError() {
    const path = Deno.makeTempDirSync() + "test";
    assertThrows(() => {
      Deno.chdir(path);
    }, Deno.errors.NotFound);
  },
);
