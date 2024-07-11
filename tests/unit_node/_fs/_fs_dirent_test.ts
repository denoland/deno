// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "@std/assert/mod.ts";
import { Dirent } from "node:fs";

class DirEntryMock implements Deno.DirEntry {
  name = "";
  isFile = false;
  isDirectory = false;
  isSymlink = false;
}

Deno.test({
  name: "Directories are correctly identified",
  fn() {
    const entry = new DirEntryMock();
    entry.isDirectory = true;
    entry.isFile = false;
    entry.isSymlink = false;
    const dir = new Dirent("foo", "parent", entry);
    assert(dir.isDirectory());
    assert(!dir.isFile());
    assert(!dir.isSymbolicLink());
  },
});

Deno.test({
  name: "Files are correctly identified",
  fn() {
    const entry = new DirEntryMock();
    entry.isDirectory = false;
    entry.isFile = true;
    entry.isSymlink = false;
    const dir = new Dirent("foo", "parent", entry);
    assert(!dir.isDirectory());
    assert(dir.isFile());
    assert(!dir.isSymbolicLink());
  },
});

Deno.test({
  name: "Symlinks are correctly identified",
  fn() {
    const entry = new DirEntryMock();
    entry.isDirectory = false;
    entry.isFile = false;
    entry.isSymlink = true;
    const dir = new Dirent("foo", "parent", entry);
    assert(!dir.isDirectory());
    assert(!dir.isFile());
    assert(dir.isSymbolicLink());
  },
});

Deno.test({
  name: "File name is correct",
  fn() {
    const entry = new DirEntryMock();
    const mock = new Dirent("my_file", "parent", entry);
    assertEquals(mock.name, "my_file");
  },
});

Deno.test({
  name: "Socket and FIFO pipes aren't yet available",
  fn() {
    const entry = new DirEntryMock();
    const dir = new Dirent("my_file", "parent", entry);
    assertThrows(
      () => dir.isFIFO(),
      Error,
      "does not yet support",
    );
    assertThrows(
      () => dir.isSocket(),
      Error,
      "does not yet support",
    );
  },
});

Deno.test({
  name: "Path and parent path is correct",
  fn() {
    const entry = new DirEntryMock();
    const dir = new Dirent("my_file", "/home/user", entry);
    assertEquals(dir.name, "my_file");
    assertEquals(dir.path, "/home/user");
    assertEquals(dir.parentPath, "/home/user");
  },
});
