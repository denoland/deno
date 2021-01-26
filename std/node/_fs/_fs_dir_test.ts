// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, fail } from "../../testing/asserts.ts";
import { assertCallbackErrorUncaught } from "../_utils.ts";
import Dir from "./_fs_dir.ts";
import type Dirent from "./_fs_dirent.ts";

Deno.test({
  name: "Closing current directory with callback is successful",
  fn() {
    let calledBack = false;
    // deno-lint-ignore no-explicit-any
    new Dir(".").close((valOrErr: any) => {
      assert(!valOrErr);
      calledBack = true;
    });
    assert(calledBack);
  },
});

Deno.test({
  name: "Closing current directory without callback returns void Promise",
  async fn() {
    await new Dir(".").close();
  },
});

Deno.test({
  name: "Closing current directory synchronously works",
  fn() {
    new Dir(".").closeSync();
  },
});

Deno.test({
  name: "Path is correctly returned",
  fn() {
    assertEquals(new Dir("std/node").path, "std/node");

    const enc: Uint8Array = new TextEncoder().encode("std/node");
    assertEquals(new Dir(enc).path, "std/node");
  },
});

Deno.test({
  name: "read returns null for empty directory",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const file: Dirent | null = await new Dir(testDir).read();
      assert(file === null);

      let calledBack = false;
      const fileFromCallback: Dirent | null = await new Dir(
        testDir,
        // deno-lint-ignore no-explicit-any
      ).read((err: any, res: Dirent) => {
        assert(res === null);
        assert(err === null);
        calledBack = true;
      });
      assert(fileFromCallback === null);
      assert(calledBack);

      assertEquals(new Dir(testDir).readSync(), null);
    } finally {
      Deno.removeSync(testDir);
    }
  },
});

Deno.test({
  name: "Async read returns one file at a time",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    const f1 = Deno.createSync(testDir + "/foo.txt");
    f1.close();
    const f2 = Deno.createSync(testDir + "/bar.txt");
    f2.close();

    try {
      let secondCallback = false;
      const dir: Dir = new Dir(testDir);
      const firstRead: Dirent | null = await dir.read();
      const secondRead: Dirent | null = await dir.read(
        // deno-lint-ignore no-explicit-any
        (err: any, secondResult: Dirent) => {
          assert(
            secondResult.name === "bar.txt" ||
              secondResult.name === "foo.txt",
          );
          secondCallback = true;
        },
      );
      const thirdRead: Dirent | null = await dir.read();
      const fourthRead: Dirent | null = await dir.read();

      if (firstRead?.name === "foo.txt") {
        assertEquals(secondRead?.name, "bar.txt");
      } else if (firstRead?.name === "bar.txt") {
        assertEquals(secondRead?.name, "foo.txt");
      } else {
        fail("File not found during read");
      }
      assert(secondCallback);
      assert(thirdRead === null);
      assert(fourthRead === null);
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "Sync read returns one file at a time",
  fn() {
    const testDir: string = Deno.makeTempDirSync();
    const f1 = Deno.createSync(testDir + "/foo.txt");
    f1.close();
    const f2 = Deno.createSync(testDir + "/bar.txt");
    f2.close();

    try {
      const dir: Dir = new Dir(testDir);
      const firstRead: Dirent | null = dir.readSync();
      const secondRead: Dirent | null = dir.readSync();
      const thirdRead: Dirent | null = dir.readSync();
      const fourthRead: Dirent | null = dir.readSync();

      if (firstRead?.name === "foo.txt") {
        assertEquals(secondRead?.name, "bar.txt");
      } else if (firstRead?.name === "bar.txt") {
        assertEquals(secondRead?.name, "foo.txt");
      } else {
        fail("File not found during read");
      }
      assert(thirdRead === null);
      assert(fourthRead === null);
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "Async iteration over existing directory",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    const f1 = Deno.createSync(testDir + "/foo.txt");
    f1.close();
    const f2 = Deno.createSync(testDir + "/bar.txt");
    f2.close();

    try {
      const dir: Dir = new Dir(testDir);
      const results: Array<string | null> = [];

      for await (const file of dir[Symbol.asyncIterator]()) {
        results.push(file.name);
      }

      assert(results.length === 2);
      assert(results.includes("foo.txt"));
      assert(results.includes("bar.txt"));
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test("[std/node/fs] Dir.close callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const importUrl = new URL("./_fs_dir.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `
    import Dir from ${JSON.stringify(importUrl)};

    const dir = new Dir(${JSON.stringify(tempDir)});
    `,
    invocation: "dir.close(",
    async cleanup() {
      await Deno.remove(tempDir);
    },
  });
});

Deno.test("[std/node/fs] Dir.read callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const importUrl = new URL("./_fs_dir.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `
    import Dir from ${JSON.stringify(importUrl)};

    const dir = new Dir(${JSON.stringify(tempDir)});
    `,
    invocation: "dir.read(",
    async cleanup() {
      await Deno.remove(tempDir);
    },
  });
});
