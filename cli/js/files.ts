// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  EOF,
  Reader,
  Writer,
  Seeker,
  Closer,
  SeekMode,
  SyncReader,
  SyncWriter,
  SyncSeeker,
} from "./io.ts";
import { close } from "./ops/resources.ts";
import { read, readSync, write, writeSync } from "./ops/io.ts";
import { seek, seekSync } from "./ops/fs/seek.ts";
export { seek, seekSync } from "./ops/fs/seek.ts";
import {
  open as opOpen,
  openSync as opOpenSync,
  OpenOptions,
  OpenMode,
} from "./ops/fs/open.ts";
export { OpenOptions, OpenMode } from "./ops/fs/open.ts";

export function openSync(path: string, options?: OpenOptions): File;
export function openSync(path: string, openMode?: OpenMode): File;

/**@internal*/
export function openSync(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): File {
  let openMode = undefined;
  let options = undefined;

  if (typeof modeOrOptions === "string") {
    openMode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions as OpenOptions;
  }

  const rid = opOpenSync(path, openMode as OpenMode, options);
  return new File(rid);
}

export async function open(path: string, options?: OpenOptions): Promise<File>;
export async function open(path: string, openMode?: OpenMode): Promise<File>;

/**@internal*/
export async function open(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): Promise<File> {
  let openMode = undefined;
  let options = undefined;

  if (typeof modeOrOptions === "string") {
    openMode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions as OpenOptions;
  }

  const rid = await opOpen(path, openMode as OpenMode, options);
  return new File(rid);
}

export function createSync(path: string): File {
  return openSync(path, "w+");
}

export function create(path: string): Promise<File> {
  return open(path, "w+");
}

export class File
  implements
    Reader,
    SyncReader,
    Writer,
    SyncWriter,
    Seeker,
    SyncSeeker,
    Closer {
  constructor(readonly rid: number) {}

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  writeSync(p: Uint8Array): number {
    return writeSync(this.rid, p);
  }

  read(p: Uint8Array): Promise<number | EOF> {
    return read(this.rid, p);
  }

  readSync(p: Uint8Array): number | EOF {
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

export const stdin = new File(0);
export const stdout = new File(1);
export const stderr = new File(2);

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
