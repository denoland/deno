// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { StatResponse } from "./ops/fs/stat.ts";
import { build } from "./build.ts";

export interface FileInfo {
  size: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;
  dev: number | null;
  ino: number | null;
  mode: number | null;
  nlink: number | null;
  uid: number | null;
  gid: number | null;
  rdev: number | null;
  blksize: number | null;
  blocks: number | null;
  isFile(): boolean;
  isDirectory(): boolean;
  isSymlink(): boolean;
}

// @internal
export class FileInfoImpl implements FileInfo {
  readonly #isFile: boolean;
  readonly #isSymlink: boolean;
  size: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;

  dev: number | null;
  ino: number | null;
  mode: number | null;
  nlink: number | null;
  uid: number | null;
  gid: number | null;
  rdev: number | null;
  blksize: number | null;
  blocks: number | null;

  /* @internal */
  constructor(res: StatResponse) {
    const isUnix = build.os === "mac" || build.os === "linux";
    const modified = res.modified;
    const accessed = res.accessed;
    const created = res.created;
    const name = res.name;
    // Unix only
    const { dev, ino, mode, nlink, uid, gid, rdev, blksize, blocks } = res;

    this.#isFile = res.isFile;
    this.#isSymlink = res.isSymlink;
    this.size = res.size;
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    this.name = name ? name : null;
    // Only non-null if on Unix
    this.dev = isUnix ? dev : null;
    this.ino = isUnix ? ino : null;
    this.mode = isUnix ? mode : null;
    this.nlink = isUnix ? nlink : null;
    this.uid = isUnix ? uid : null;
    this.gid = isUnix ? gid : null;
    this.rdev = isUnix ? rdev : null;
    this.blksize = isUnix ? blksize : null;
    this.blocks = isUnix ? blocks : null;
  }

  isFile(): boolean {
    return this.#isFile;
  }

  isDirectory(): boolean {
    return !this.#isFile && !this.#isSymlink;
  }

  isSymlink(): boolean {
    return this.#isSymlink;
  }
}
