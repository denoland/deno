// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "@std/assert/mod.ts";
import { Dirent as Dirent_ } from "node:fs";

// deno-lint-ignore no-explicit-any
const Dirent = Dirent_ as any;

class DirEntryMock implements Deno.DirEntry {
  parentPath = "";
  name = "";
  isFile = false;
  isDirectory = false;
  isSymlink = false;
}

Deno.test({
  name: "Directories are correctly identified",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    entry.isDirectory = true;
    entry.isFile = false;
    entry.isSymlink = false;
    assert(new Dirent(entry).isDirectory());
    assert(!new Dirent(entry).isFile());
    assert(!new Dirent(entry).isSymbolicLink());
  },
});

Deno.test({
  name: "Files are correctly identified",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    entry.isDirectory = false;
    entry.isFile = true;
    entry.isSymlink = false;
    assert(!new Dirent(entry).isDirectory());
    assert(new Dirent(entry).isFile());
    assert(!new Dirent(entry).isSymbolicLink());
  },
});

Deno.test({
  name: "Symlinks are correctly identified",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    entry.isDirectory = false;
    entry.isFile = false;
    entry.isSymlink = true;
    assert(!new Dirent(entry).isDirectory());
    assert(!new Dirent(entry).isFile());
    assert(new Dirent(entry).isSymbolicLink());
  },
});

Deno.test({
  name: "File name is correct",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    entry.name = "my_file";
    assertEquals(new Dirent(entry).name, "my_file");
  },
});

Deno.test({
  name: "Socket and FIFO pipes aren't yet available",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    assertThrows(
      () => {
        new Dirent(entry).isFIFO();
      },
      Error,
      "does not yet support",
    );
    assertThrows(
      () => {
        new Dirent(entry).isSocket();
      },
      Error,
      "does not yet support",
    );
  },
});

Deno.test({
  name: "Path and parent path is correct",
  fn() {
    const entry: DirEntryMock = new DirEntryMock();
    entry.name = "my_file";
    entry.parentPath = "/home/user";
    assertEquals(new Dirent(entry).name, "my_file");
    assertEquals(new Dirent(entry).path, "/home/user");
    assertEquals(new Dirent(entry).parentPath, "/home/user");
  },
});
