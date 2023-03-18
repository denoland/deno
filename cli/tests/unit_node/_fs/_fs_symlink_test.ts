// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows, fail } from "../../testing/asserts.ts";
import { symlink, symlinkSync } from "./_fs_symlink.ts";

Deno.test({
  name: "ASYNC: no callback function results in Error",
  fn() {
    assertThrows(
      () => {
        symlink("some/path", "some/other/path", "dir");
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "ASYNC: create symlink point to a file",
  async fn() {
    const file: string = Deno.makeTempFileSync();
    const linkedFile: string = file + ".link";

    await new Promise<void>((resolve, reject) => {
      symlink(file, linkedFile, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const stat = Deno.lstatSync(linkedFile);
          assert(stat.isSymlink);
        },
        () => {
          fail("Expected to succeed");
        },
      )
      .finally(() => {
        Deno.removeSync(file);
        Deno.removeSync(linkedFile);
      });
  },
});

Deno.test({
  name: "ASYNC: create symlink point to a dir",
  async fn() {
    const dir: string = Deno.makeTempDirSync();
    const linkedDir: string = dir + ".link";

    await new Promise<void>((resolve, reject) => {
      symlink(dir, linkedDir, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const stat = Deno.lstatSync(linkedDir);
          assert(stat.isSymlink);
        },
        () => {
          fail("Expected to succeed");
        },
      )
      .finally(() => {
        Deno.removeSync(dir);
        Deno.removeSync(linkedDir);
      });
  },
});

Deno.test({
  name: "SYNC: create symlink point to a file",
  fn() {
    const file: string = Deno.makeTempFileSync();
    const linkedFile: string = file + ".link";

    try {
      symlinkSync(file, linkedFile);
      const stat = Deno.lstatSync(linkedFile);
      assert(stat.isSymlink);
    } finally {
      Deno.removeSync(file);
      Deno.removeSync(linkedFile);
    }
  },
});

Deno.test({
  name: "SYNC: create symlink point to a dir",
  fn() {
    const dir: string = Deno.makeTempDirSync();
    const linkedDir: string = dir + ".link";

    try {
      symlinkSync(dir, linkedDir);
      const stat = Deno.lstatSync(linkedDir);
      assert(stat.isSymlink);
    } finally {
      Deno.removeSync(dir);
      Deno.removeSync(linkedDir);
    }
  },
});
