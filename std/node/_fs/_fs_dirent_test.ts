const { test } = Deno;
import { assert, assertEquals, assertThrows } from "../../testing/asserts.ts";
import Dirent from "./_fs_dirent.ts";

class FileInfoMock implements Deno.FileInfo {
  size = -1;
  modified = -1;
  accessed = -1;
  created = -1;
  name = "";
  dev = -1;
  ino = -1;
  mode = -1;
  nlink = -1;
  uid = -1;
  gid = -1;
  rdev = -1;
  blksize = -1;
  blocks: number | null = null;

  isFileMock = false;
  isDirectoryMock = false;
  isSymlinkMock = false;

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
  },
});

test({
  name: "Character devices are correctly identified",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.blocks = null;
    assert(new Dirent(fileInfo).isCharacterDevice());
    assert(!new Dirent(fileInfo).isBlockDevice());
  },
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
  },
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
  },
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
  },
});

test({
  name: "File name is correct",
  fn() {
    const fileInfo: FileInfoMock = new FileInfoMock();
    fileInfo.name = "my_file";
    assertEquals(new Dirent(fileInfo).name, "my_file");
  },
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
  },
});
