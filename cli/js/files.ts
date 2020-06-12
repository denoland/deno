// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  Reader,
  Writer,
  Seeker,
  Closer,
  SeekMode,
  ReaderSync,
  WriterSync,
  SeekerSync,
} from "./io.ts";
import { close } from "./ops/resources.ts";
import { read, readSync, write, writeSync } from "./ops/io.ts";
import { seek, seekSync } from "./ops/fs/seek.ts";
export { seek, seekSync } from "./ops/fs/seek.ts";
import {
  open as opOpen,
  openSync as opOpenSync,
  OpenOptions,
} from "./ops/fs/open.ts";
export { OpenOptions } from "./ops/fs/open.ts";

export function openSync(
  path: string | URL,
  options: OpenOptions = { read: true }
): File {
  checkOpenOptions(options);
  const rid = opOpenSync(path, options);
  return new File(rid);
}

export async function open(
  path: string | URL,
  options: OpenOptions = { read: true }
): Promise<File> {
  checkOpenOptions(options);
  const rid = await opOpen(path, options);
  return new File(rid);
}

export function createSync(path: string | URL): File {
  return openSync(path, {
    read: true,
    write: true,
    truncate: true,
    create: true,
  });
}

export function create(path: string | URL): Promise<File> {
  return open(path, {
    read: true,
    write: true,
    truncate: true,
    create: true,
  });
}

export class File
  implements
    Reader,
    ReaderSync,
    Writer,
    WriterSync,
    Seeker,
    SeekerSync,
    Closer {
  constructor(readonly rid: number) {}

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  writeSync(p: Uint8Array): number {
    return writeSync(this.rid, p);
  }

  read(p: Uint8Array): Promise<number | null> {
    return read(this.rid, p);
  }

  readSync(p: Uint8Array): number | null {
    return readSync(this.rid, p);
  }

  seek(offset: number, whence: SeekMode): Promise<number> {
    return seek(this.rid, offset, whence);
  }

  seekSync(offset: number, whence: SeekMode): number {
    return seekSync(this.rid, offset, whence);
  }

  close(): void {
    close(this.rid);
  }
}

class Stdin implements Reader, ReaderSync, Closer {
  readonly rid: number;
  constructor() {
    this.rid = 0;
  }

  read(p: Uint8Array): Promise<number | null> {
    return read(this.rid, p);
  }

  readSync(p: Uint8Array): number | null {
    return readSync(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }
}

class Stdout implements Writer, WriterSync, Closer {
  readonly rid: number;
  constructor() {
    this.rid = 1;
  }

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  writeSync(p: Uint8Array): number {
    return writeSync(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }
}

export class Stderr implements Writer, WriterSync, Closer {
  readonly rid: number;
  constructor() {
    this.rid = 2;
  }

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  writeSync(p: Uint8Array): number {
    return writeSync(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }
}

export const stdin = new Stdin();
export const stdout = new Stdout();
export const stderr = new Stderr();

function checkOpenOptions(options: OpenOptions): void {
  if (Object.values(options).filter((val) => val === true).length === 0) {
    throw new Error("OpenOptions requires at least one option to be true");
  }

  if (options.truncate && !options.write) {
    throw new Error("'truncate' option requires 'write' option");
  }

  const createOrCreateNewWithoutWriteOrAppend =
    (options.create || options.createNew) && !(options.write || options.append);

  if (createOrCreateNewWithoutWriteOrAppend) {
    throw new Error(
      "'create' or 'createNew' options require 'write' or 'append' option"
    );
  }
}
