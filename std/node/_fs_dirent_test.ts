"use strict";

const { test } = Deno;
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import Dirent from "./_fs_dirent.ts";

class FileInfoMock implements Deno.FileInfo {
  len: number;
  modified: number;
  accessed: number;
  created: number;
  name: string;
  dev: number;
  ino: number;
  mode: number;
  nlink: number;
  uid: number;
  gid: number;
  rdev: number;
  blksize: number;
  blocks: number;

  isFileMock: boolean;
  isDirectoryMock: boolean;
  isSymlinkMock: boolean;

  isFile(): boolean {
    return this.isFileMock;
  }
  isDirectory(): boolean {
    return this.isDirectoryMock;
  }
  isSymlink(): boolean {
    return this.isSymlinkMock;
  }
}

test({
  name: "Block devices are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.blocks = 5;
    assert(new Dirent(fileInfo).isBlockDevice());
    assert(!new Dirent(fileInfo).isCharacterDevice());
  }
});

test({
  name: "Character devices are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.blocks = null;
    assert(new Dirent(fileInfo).isCharacterDevice());
    assert(!new Dirent(fileInfo).isBlockDevice());
  }
});

test({
  name: "Directories are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.isDirectoryMock = true;
    fileInfo.isFileMock = false;
    fileInfo.isSymlinkMock = false;
    assert(new Dirent(fileInfo).isDirectory());
    assert(!new Dirent(fileInfo).isFile());
    assert(!new Dirent(fileInfo).isSymbolicLink());
  }
});

test({
  name: "Files are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.isDirectoryMock = false;
    fileInfo.isFileMock = true;
    fileInfo.isSymlinkMock = false;
    assert(!new Dirent(fileInfo).isDirectory());
    assert(new Dirent(fileInfo).isFile());
    assert(!new Dirent(fileInfo).isSymbolicLink());
  }
});

test({
  name: "Symlinks are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.isDirectoryMock = false;
    fileInfo.isFileMock = false;
    fileInfo.isSymlinkMock = true;
    assert(!new Dirent(fileInfo).isDirectory());
    assert(!new Dirent(fileInfo).isFile());
    assert(new Dirent(fileInfo).isSymbolicLink());
  }
});

test({
  name: "File name is correct",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.name = "my_file";
    assertEquals(new Dirent(fileInfo).name, "my_file");
  }
});

test({
  name: "Socket and FIFO pipes aren't yet available",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    assertThrows(
      () => {
        new Dirent(fileInfo).isFIFO();
      },
      Error,
      "does not yet support"
    );
    assertThrows(
      () => {
        new Dirent(fileInfo).isSocket();
      },
      Error,
      "does not yet support"
    );
  }
});
