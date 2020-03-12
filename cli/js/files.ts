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
  SyncSeeker
} from "./io.ts";
import { close } from "./ops/resources.ts";
import { read, readSync, write, writeSync } from "./ops/io.ts";
import { seek, seekSync } from "./ops/fs/seek.ts";
export { seek, seekSync } from "./ops/fs/seek.ts";
import {
  open as opOpen,
  openSync as opOpenSync,
  OpenOptions,
  OpenMode
} from "./ops/fs/open.ts";
export { OpenOptions, OpenMode } from "./ops/fs/open.ts";

/** Synchronously open a file and return an instance of the `File` object.
 *
 *       const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export function openSync(path: string, mode?: OpenOptions): File;
export function openSync(path: string, mode?: OpenMode): File;
export function openSync(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): File {
  let mode = undefined;
  let options = undefined;

  if (typeof modeOrOptions === "string") {
    mode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions as OpenOptions;
  }

  const rid = opOpenSync(path, mode as OpenMode, options);
  return new File(rid);
}

/** Open a file and resolve to an instance of the `File` object.
 *
 *     const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export async function open(path: string, options?: OpenOptions): Promise<File>;
export async function open(path: string, mode?: OpenMode): Promise<File>;
export async function open(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): Promise<File> {
  let mode = undefined;
  let options = undefined;

  if (typeof modeOrOptions === "string") {
    mode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions as OpenOptions;
  }

  const rid = await opOpen(path, mode as OpenMode, options);
  return new File(rid);
}

/** Creates a file if none exists or truncates an existing file and returns
 *  an instance of `Deno.File`.
 *
 *       const file = Deno.createSync("/foo/bar.txt");
 *
 * Requires `allow-read` and `allow-write` permissions.
 */
export function createSync(path: string): File {
  return openSync(path, "w+");
}

/** Creates a file if none exists or truncates an existing file and resolves to
 *  an instance of `Deno.File`.
 *
 *       const file = await Deno.create("/foo/bar.txt");
 *
 * Requires `allow-read` and `allow-write` permissions.
 */
export function create(path: string): Promise<File> {
  return open(path, "w+");
}

/** The Deno abstraction for reading and writing files. */
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

/** An instance of `Deno.File` for `stdin`. */
export const stdin = new File(0);
/** An instance of `Deno.File` for `stdout`. */
export const stdout = new File(1);
/** An instance of `Deno.File` for `stderr`. */
export const stderr = new File(2);

/** Check if OpenOptions is set to valid combination of options.
 *  @returns Tuple representing if openMode is valid and error message if it's not
 *  @internal
 */
function checkOpenOptions(options: OpenOptions): void {
  if (Object.values(options).filter(val => val === true).length === 0) {
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
