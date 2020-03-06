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
import { sendAsyncMinimal, sendSyncMinimal } from "./dispatch_minimal.ts";
import {
  sendSync as sendSyncJson,
  sendAsync as sendAsyncJson
} from "./dispatch_json.ts";
import { OPS_CACHE } from "./runtime.ts";

// This is done because read/write are extremely performance sensitive.
let OP_READ = -1;
let OP_WRITE = -1;

/** Synchronously open a file and return an instance of the `File` object.
 *
 *       const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export function openSync(path: string, mode?: OpenOptions): File;

/** Synchronously open a file and return an instance of the `File` object.
 *
 *       const file = Deno.openSync("/foo/bar.txt", "r");
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export function openSync(path: string, mode?: OpenMode): File;

/**@internal*/
export function openSync(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): File {
  let mode = null;
  let options = null;

  if (typeof modeOrOptions === "string") {
    mode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions;
  }

  const rid = sendSyncJson("op_open", { path, options, mode });
  return new File(rid);
}

/** Open a file and resolve to an instance of the `File` object.
 *
 *     const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export async function open(path: string, options?: OpenOptions): Promise<File>;

/** Open a file and resolves to an instance of `Deno.File`.
 *
 *     const file = await Deno.open("/foo/bar.txt, "w+");
 *
 * Requires `allow-read` and `allow-write` permissions depending on mode.
 */
export async function open(path: string, mode?: OpenMode): Promise<File>;

/**@internal*/
export async function open(
  path: string,
  modeOrOptions: OpenOptions | OpenMode = "r"
): Promise<File> {
  let mode = null;
  let options = null;

  if (typeof modeOrOptions === "string") {
    mode = modeOrOptions;
  } else {
    checkOpenOptions(modeOrOptions);
    options = modeOrOptions;
  }

  const rid = await sendAsyncJson("op_open", {
    path,
    options,
    mode
  });
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

/** Synchronously read from a file ID into an array buffer.
 *
 * Returns `number | EOF` for the operation.
 *
 *      const file = Deno.openSync("/foo/bar.txt");
 *      const buf = new Uint8Array(100);
 *      const nread = Deno.readSync(file.rid, buf);
 *      const text = new TextDecoder().decode(buf);
 */
export function readSync(rid: number, p: Uint8Array): number | EOF {
  if (p.length == 0) {
    return 0;
  }
  if (OP_READ < 0) {
    OP_READ = OPS_CACHE["op_read"];
  }
  const nread = sendSyncMinimal(OP_READ, rid, p);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return EOF;
  } else {
    return nread;
  }
}

/** Read from a resource ID into an array buffer.
 *
 * Resolves to the `number | EOF` for the operation.
 *
 *       const file = await Deno.open("/foo/bar.txt");
 *       const buf = new Uint8Array(100);
 *       const nread = await Deno.read(file.rid, buf);
 *       const text = new TextDecoder().decode(buf);
 */
export async function read(rid: number, p: Uint8Array): Promise<number | EOF> {
  if (p.length == 0) {
    return 0;
  }
  if (OP_READ < 0) {
    OP_READ = OPS_CACHE["op_read"];
  }
  const nread = await sendAsyncMinimal(OP_READ, rid, p);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return EOF;
  } else {
    return nread;
  }
}

/** Synchronously write to the resource ID the contents of the array buffer.
 *
 * Resolves to the number of bytes written.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       const file = Deno.openSync("/foo/bar.txt", {create: true, write: true});
 *       Deno.writeSync(file.rid, data);
 */
export function writeSync(rid: number, p: Uint8Array): number {
  if (OP_WRITE < 0) {
    OP_WRITE = OPS_CACHE["op_write"];
  }
  const result = sendSyncMinimal(OP_WRITE, rid, p);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

/** Write to the resource ID the contents of the array buffer.
 *
 * Resolves to the number of bytes written.
 *
 *      const encoder = new TextEncoder();
 *      const data = encoder.encode("Hello world\n");
 *      const file = await Deno.open("/foo/bar.txt", {create: true, write: true});
 *      await Deno.write(file.rid, data);
 */
export async function write(rid: number, p: Uint8Array): Promise<number> {
  if (OP_WRITE < 0) {
    OP_WRITE = OPS_CACHE["op_write"];
  }
  const result = await sendAsyncMinimal(OP_WRITE, rid, p);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

/** Synchronously seek a file ID to the given offset under mode given by `whence`.
 *
 * Returns the number of cursor position.
 *
 *       const file = Deno.openSync("/foo/bar.txt");
 *       const position = Deno.seekSync(file.rid, 0, 0);
 */
export function seekSync(
  rid: number,
  offset: number,
  whence: SeekMode
): number {
  return sendSyncJson("op_seek", { rid, offset, whence });
}

/** Seek a file ID to the given offset under mode given by `whence`.
 *
 * Resolves with the number of cursor position.
 *
 *      const file = await Deno.open("/foo/bar.txt");
 *      const position = await Deno.seek(file.rid, 0, 0);
 */
export async function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<number> {
  return await sendAsyncJson("op_seek", { rid, offset, whence });
}

/** Close the given resource ID. */
export function close(rid: number): void {
  sendSyncJson("op_close", { rid });
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

export interface OpenOptions {
  /** Sets the option for read access. This option, when `true`, means that the
   * file should be read-able if opened. */
  read?: boolean;
  /** Sets the option for write access. This option, when `true`, means that
   * the file should be write-able if opened. If the file already exists,
   * any write calls on it will overwrite its contents, by default without
   * truncating it. */
  write?: boolean;
  /**Sets the option for the append mode. This option, when `true`, means that
   * writes will append to a file instead of overwriting previous contents.
   * Note that setting `{ write: true, append: true }` has the same effect as
   * setting only `{ append: true }`. */
  append?: boolean;
  /** Sets the option for truncating a previous file. If a file is
   * successfully opened with this option set it will truncate the file to `0`
   * length if it already exists. The file must be opened with write access
   * for truncate to work. */
  truncate?: boolean;
  /** Sets the option to allow creating a new file, if one doesn't already
   * exist at the specified path. Requires write or append access to be
   * used. */
  create?: boolean;
  /** Defaults to `false`. If set to `true`, no file, directory, or symlink is
   * allowed to exist at the target location. Requires write or append
   * access to be used. When createNew is set to `true`, create and truncate
   * are ignored. */
  createNew?: boolean;
}

/** A set of string literals which specify the open mode of a file.
 *
 * |Value |Description                                                                                       |
 * |------|--------------------------------------------------------------------------------------------------|
 * |`"r"` |Read-only. Default. Starts at beginning of file.                                                  |
 * |`"r+"`|Read-write. Start at beginning of file.                                                           |
 * |`"w"` |Write-only. Opens and truncates existing file or creates new one for writing only.                |
 * |`"w+"`|Read-write. Opens and truncates existing file or creates new one for writing and reading.         |
 * |`"a"` |Write-only. Opens existing file or creates new one. Each write appends content to the end of file.|
 * |`"a+"`|Read-write. Behaves like `"a"` and allows to read from file.                                      |
 * |`"x"` |Write-only. Exclusive create - creates new file only if one doesn't exist already.                |
 * |`"x+"`|Read-write. Behaves like `x` and allows reading from file.                                        |
 */
export type OpenMode = "r" | "r+" | "w" | "w+" | "a" | "a+" | "x" | "x+";

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
