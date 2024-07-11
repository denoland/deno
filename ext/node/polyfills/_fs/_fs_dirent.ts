// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "ext:deno_node/_utils.ts";

export default class Dirent {
  constructor(
    // This is the most frequently accessed property. Using a non-getter
    // is a very tiny bit faster here
    public name: string,
    public parentPath: string,
    private entry: Deno.DirEntry,
  ) {
  }

  isBlockDevice(): boolean {
    notImplemented("Deno does not yet support identification of block devices");
    return false;
  }

  isCharacterDevice(): boolean {
    notImplemented(
      "Deno does not yet support identification of character devices",
    );
    return false;
  }

  isDirectory(): boolean {
    return this.entry.isDirectory;
  }

  isFIFO(): boolean {
    notImplemented(
      "Deno does not yet support identification of FIFO named pipes",
    );
    return false;
  }

  isFile(): boolean {
    return this.entry.isFile;
  }

  isSocket(): boolean {
    notImplemented("Deno does not yet support identification of sockets");
    return false;
  }

  isSymbolicLink(): boolean {
    return this.entry.isSymlink;
  }

  /** @deprecated */
  get path(): string {
    return this.parentPath;
  }
}
