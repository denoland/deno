// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";

Deno.test("dirCwdNotNull", function (): void {
  assert(Deno.cwd() != null);
});

Deno.test(
  "dirCwdChdirSuccess",
  function (): void {
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

Deno.test({
  name: "dirCwdError",
  // excluding windows since it throws resource busy, while removeSync
  ignore: Deno.build.os == "windows",
  fn(): void {
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
  },
});

Deno.test(
  "dirChdirError",
  function (): void {
    const path = Deno.makeTempDirSync() + "test";
    assertThrows(() => {
      Deno.chdir(path);
    }, Deno.errors.NotFound);
  },
);

Deno.test("dirCwdPermError", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(
    () => {
      Deno.cwd();
    },
    Deno.errors.PermissionDenied,
    "Requires read access to <CWD>, run again with the --allow-read flag",
  );
});
