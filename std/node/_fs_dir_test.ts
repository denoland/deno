"use strict";

const { test } = Deno;
import {
  assert,
  assertEquals,
  fail,
  assertThrows
} from "../testing/asserts.ts";
import Dir from "./_fs_dir.ts";
import Dirent from "./_fs_dirent.ts";

function neverCalledBack(): never {
  throw new Error("This should never be called");
}

test({
  name: "Closing a non-existant directory throws an error",
  fn() {
    assertThrows(
      () => {
        new Dir(999999999, ".").close(neverCalledBack);
      },
      Error,
      "Directory handle was closed"
    );
  }
});

test({
  name: "Closing current directory with callback is successful",
  async fn() {
    const fileInfo: Deno.File = await Deno.open(".");
    assert(Deno.resources()[fileInfo.rid]);
    let calledBack = false;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    new Dir(fileInfo.rid, ".").close((valOrErr: any) => {
      assert(!valOrErr);
      calledBack = true;
    });
    assert(calledBack);
    assert(!Deno.resources()[fileInfo.rid]);
  }
});

test({
  name: "Closing current directory without callback returns Promise",
  async fn() {
    const fileInfo: Deno.File = await Deno.open(".");
    assert(Deno.resources()[fileInfo.rid]);
    await new Dir(fileInfo.rid, ".")
      .close()
      .then(() => {
        assert(!Deno.resources()[fileInfo.rid]);
      })
      .catch(() => {
        fail("Unexpected error closing resource");
      });
  }
});

test({
  name: "Closing current directory synchronously works",
  async fn() {
    const fileInfo: Deno.File = await Deno.open(".");
    assert(Deno.resources()[fileInfo.rid]);
    new Dir(fileInfo.rid, ".").closeSync();
    assert(!Deno.resources()[fileInfo.rid]);
  }
});

test({
  name: "Path is correctly returned",
  fn() {
    assertEquals(new Dir(1, "std/node").path, "std/node");

    const enc: Uint8Array = new TextEncoder().encode("std/node");
    assertEquals(new Dir(1, enc).path, "std/node");
  }
});

test({
  name: "Async read fails if directory isn't open",
  async fn() {
    assertThrows(
      () => {
        new Dir(-1, "std/node").read();
      },
      Error,
      "Directory handle was closed"
    );
    assertThrows(
      () => {
        new Dir(-1, "std/node").read(neverCalledBack);
      },
      Error,
      "Directory handle was closed"
    );
  }
});

test({
  name: "read returns null for empty directory",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    try {
      const fileInfo: Deno.File = await Deno.open(testDir);
      const file: Dirent | null = await new Dir(fileInfo.rid, testDir).read();
      assert(file === null);

      let calledBack = false;
      const fileFromCallback: Dirent | null = await new Dir(
        fileInfo.rid,
        testDir
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ).read((err: any, res: Dirent) => {
        assert(res === null);
        assert(err === null);
        calledBack = true;
      });
      assert(fileFromCallback === null);
      assert(calledBack);

      assertEquals(new Dir(fileInfo.rid, testDir).readSync(), null);
    } finally {
      Deno.removeSync(testDir);
    }
  }
});

test({
  name: "Async read returns one file at a time",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    Deno.createSync(testDir + "/foo.txt");
    Deno.createSync(testDir + "/bar.txt");

    try {
      let secondCallback = false;
      const fileInfo: Deno.File = await Deno.open(testDir);
      const dir: Dir = new Dir(fileInfo.rid, testDir);
      const firstRead: Dirent | null = await dir.read();
      const secondRead: Dirent | null = await dir.read(
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (err: any, secondResult: Dirent) => {
          assert(
            secondResult.name === "bar.txt" || secondResult.name === "foo.txt"
          );
          secondCallback = true;
        }
      );
      const thirdRead: Dirent | null = await dir.read();

      if (firstRead?.name === "foo.txt") {
        assertEquals(secondRead?.name, "bar.txt");
      } else if (firstRead?.name === "bar.txt") {
        assertEquals(secondRead?.name, "foo.txt");
      } else {
        fail("File not found during read");
      }
      assert(secondCallback);
      assert(thirdRead === null);
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  }
});

test({
  name: "Sync read returns one file at a time",
  fn() {
    const testDir: string = Deno.makeTempDirSync();
    Deno.createSync(testDir + "/foo.txt");
    Deno.createSync(testDir + "/bar.txt");

    try {
      const fileInfo: Deno.File = Deno.openSync(testDir);
      const dir: Dir = new Dir(fileInfo.rid, testDir);
      const firstRead: Dirent | null = dir.readSync();
      const secondRead: Dirent | null = dir.readSync();
      const thirdRead: Dirent | null = dir.readSync();

      if (firstRead?.name === "foo.txt") {
        assertEquals(secondRead?.name, "bar.txt");
      } else if (firstRead?.name === "bar.txt") {
        assertEquals(secondRead?.name, "foo.txt");
      } else {
        fail("File not found during read");
      }
      assert(thirdRead === null);
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  }
});

test({
  name: "Async iteration over existing directory",
  async fn() {
    const testDir: string = Deno.makeTempDirSync();
    Deno.createSync(testDir + "/foo.txt");
    Deno.createSync(testDir + "/bar.txt");

    try {
      const fileInfo: Deno.File = Deno.openSync(testDir);
      const dir: Dir = new Dir(fileInfo.rid, testDir);
      const results: string[] = [];

      for await (const file of dir.entries()) {
        results.push(file.name);
      }

      assert(results.length === 2);
      assert(results.includes("foo.txt"));
      assert(results.includes("bar.txt"));
    } finally {
      Deno.removeSync(testDir, { recursive: true });
    }
  }
});
