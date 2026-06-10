// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { Dir as DirOrig, type Dirent } from "node:fs";

// deno-lint-ignore no-explicit-any
const Dir = DirOrig as any;

Deno.test({
  name: "Closing current directory with callback is successful",
  // Match node: close(callback) returns undefined and invokes the callback
  // asynchronously (process.nextTick).
  async fn() {
    await new Promise<void>((resolve, reject) => {
      // deno-lint-ignore no-explicit-any
      const ret = new Dir(".").close((valOrErr: any) => {
        try {
          assert(!valOrErr);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
      assert(ret === undefined);
    });
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

      // Match node: read(callback) returns undefined; the result arrives via
      // the callback only.
      await new Promise<void>((resolve, reject) => {
        const ret = new Dir(testDir)
          // deno-lint-ignore no-explicit-any
          .read((err: any, res: Dirent | null) => {
            try {
              assert(res === null);
              assert(err === null);
              resolve();
            } catch (e) {
              reject(e);
            }
          });
        assert(ret === undefined);
      });

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
      const dir = new Dir(testDir);
      const firstRead: Dirent | null = await dir.read();
      // Match node: read(callback) returns undefined; the dirent arrives via
      // the callback only.
      const secondRead: Dirent = await new Promise((resolve, reject) => {
        const ret = dir.read(
          // deno-lint-ignore no-explicit-any
          (err: any, secondResult: Dirent) => {
            if (err) reject(err);
            else resolve(secondResult);
          },
        );
        assert(ret === undefined);
      });
      const thirdRead: Dirent | null = await dir.read();
      const fourthRead: Dirent | null = await dir.read();

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
  name: "Sync read returns one file at a time",
  fn() {
    const testDir: string = Deno.makeTempDirSync();
    const f1 = Deno.createSync(testDir + "/foo.txt");
    f1.close();
    const f2 = Deno.createSync(testDir + "/bar.txt");
    f2.close();

    try {
      const dir = new Dir(testDir);
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
      const dir = new Dir(testDir);
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

Deno.test(
  "[std/node/fs] Dir.close callback isn't called twice if error is thrown",
  async () => {
    const tempDir = await Deno.makeTempDir();
    await assertCallbackErrorUncaught({
      prelude: `
    import { Dir } from "node:fs";

    const dir = new Dir(${JSON.stringify(tempDir)});
    `,
      invocation: "dir.close(",
      async cleanup() {
        await Deno.remove(tempDir);
      },
    });
  },
);

Deno.test(
  "[std/node/fs] Dir.read callback isn't called twice if error is thrown",
  async () => {
    const tempDir = await Deno.makeTempDir();
    await assertCallbackErrorUncaught({
      prelude: `
    import { Dir } from "node:fs";

    const dir = new Dir(${JSON.stringify(tempDir)});
    `,
      invocation: "dir.read(",
      async cleanup() {
        await Deno.remove(tempDir);
      },
    });
  },
);

Deno.test({
  name: "Dir.readSync throws ERR_DIR_CLOSED after closeSync",
  fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const dir = new Dir(testDir);
      dir.closeSync();
      try {
        dir.readSync();
        fail("Expected ERR_DIR_CLOSED to be thrown");
      } catch (e) {
        // deno-lint-ignore no-explicit-any
        assertEquals((e as any).code, "ERR_DIR_CLOSED");
      }
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "Dir.read rejects with ERR_DIR_CLOSED after close",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const dir = new Dir(testDir);
      await dir.close();
      try {
        await dir.read();
        fail("Expected ERR_DIR_CLOSED to be thrown");
      } catch (e) {
        // deno-lint-ignore no-explicit-any
        assertEquals((e as any).code, "ERR_DIR_CLOSED");
      }
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "Dir.readSync throws ERR_DIR_CLOSED after async close",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const dir = new Dir(testDir);
      await dir.close();
      try {
        dir.readSync();
        fail("Expected ERR_DIR_CLOSED to be thrown");
      } catch (e) {
        // deno-lint-ignore no-explicit-any
        assertEquals((e as any).code, "ERR_DIR_CLOSED");
      }
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});

Deno.test({
  // Match node: read(callback) on a closed Dir throws synchronously; the
  // callback is never invoked.
  name: "Dir.read with callback throws ERR_DIR_CLOSED synchronously",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const dir = new Dir(testDir);
      await dir.close();
      try {
        dir.read(() => fail("callback should not be invoked"));
        fail("Expected ERR_DIR_CLOSED to be thrown");
      } catch (e) {
        // deno-lint-ignore no-explicit-any
        assertEquals((e as any).code, "ERR_DIR_CLOSED");
      }
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  },
});
