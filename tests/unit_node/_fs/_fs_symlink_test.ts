// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, assertThrows, fail } from "@std/assert";
import { symlink, symlinkSync } from "node:fs";

Deno.test({
  name: "ASYNC: no callback function results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'string' is not assignable to parameter of type 'NoParamCallback'
        symlink("some/path", "some/other/path", "dir");
      },
      Error,
      "The \"cb\" argument must be of type function. Received type string ('dir')",
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

Deno.test({
  name: "SYNC: symlink junction",
  fn() {
    const dir: string = Deno.makeTempDirSync();
    const linkedDir: string = dir + "-junction";

    try {
      symlinkSync(dir, linkedDir, "junction");
      const stat = Deno.lstatSync(linkedDir);
      assert(stat.isSymlink);
    } finally {
      Deno.removeSync(dir);
      Deno.removeSync(linkedDir);
    }
  },
});
