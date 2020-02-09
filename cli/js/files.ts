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
import * as dispatch from "./dispatch.ts";
import {
  sendSync as sendSyncJson,
  sendAsync as sendAsyncJson
} from "./dispatch_json.ts";

/** Open a file and return an instance of the `File` object
 *  synchronously.
 *
 *       const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
 */
export function openSync(filename: string, capability?: OpenOptions): File;
/** Open a file and return an instance of the `File` object
 *  synchronously.
 *
 *       const file = Deno.openSync("/foo/bar.txt", "r");
 */
export function openSync(filename: string, mode?: OpenMode): File;

export function openSync(
  filename: string,
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

  const rid = sendSyncJson(dispatch.OP_OPEN, { filename, options, mode });
  return new File(rid);
}

/** Open a file and return an instance of the `File` object.
 *
 *     const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
 */
export async function open(
  filename: string,
  options?: OpenOptions
): Promise<File>;

/** Open a file and return an instance of the `File` object.
 *
 *     const file = await Deno.open("/foo/bar.txt, "w+");
 */
export async function open(filename: string, mode?: OpenMode): Promise<File>;

/**@internal*/
export async function open(
  filename: string,
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

  const rid = await sendAsyncJson(dispatch.OP_OPEN, {
    filename,
    options,
    mode
  });
  return new File(rid);
}

/** Creates a file if none exists or truncates an existing file and returns
 *  an instance of the `File` object synchronously.
 *
 *       const file = Deno.createSync("/foo/bar.txt");
 */
export function createSync(filename: string): File {
  return openSync(filename, "w+");
}

/** Creates a file if none exists or truncates an existing file and returns
 *  an instance of the `File` object.
 *
 *       const file = await Deno.create("/foo/bar.txt");
 */
export function create(filename: string): Promise<File> {
  return open(filename, "w+");
}

/** Read synchronously from a file ID into an array buffer.
 *
 * Return `number | EOF` for the operation.
 *
 *      const file = Deno.openSync("/foo/bar.txt");
 *      const buf = new Uint8Array(100);
 *      const nread = Deno.readSync(file.rid, buf);
 *      const text = new TextDecoder().decode(buf);
 *
 */
export function readSync(rid: number, p: Uint8Array): number | EOF {
  if (p.length == 0) {
    return 0;
  }
  const nread = sendSyncMinimal(dispatch.OP_READ, rid, p);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return EOF;
  } else {
    return nread;
  }
}

/** Read from a file ID into an array buffer.
 *
 * Resolves with the `number | EOF` for the operation.
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
  const nread = await sendAsyncMinimal(dispatch.OP_READ, rid, p);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return EOF;
  } else {
    return nread;
  }
}

/** Write synchronously to the file ID the contents of the array buffer.
 *
 * Resolves with the number of bytes written.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       const file = Deno.openSync("/foo/bar.txt");
 *       Deno.writeSync(file.rid, data);
 */
export function writeSync(rid: number, p: Uint8Array): number {
  const result = sendSyncMinimal(dispatch.OP_WRITE, rid, p);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

/** Write to the file ID the contents of the array buffer.
 *
 * Resolves with the number of bytes written.
 *
 *      const encoder = new TextEncoder();
 *      const data = encoder.encode("Hello world\n");
 *      const file = await Deno.open("/foo/bar.txt");
 *      await Deno.write(file.rid, data);
 *
 */
export async function write(rid: number, p: Uint8Array): Promise<number> {
  const result = await sendAsyncMinimal(dispatch.OP_WRITE, rid, p);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

/** Seek a file ID synchronously to the given offset under mode given by `whence`.
 *
 *       const file = Deno.openSync("/foo/bar.txt");
 *       Deno.seekSync(file.rid, 0, 0);
 */
export function seekSync(rid: number, offset: number, whence: SeekMode): void {
  sendSyncJson(dispatch.OP_SEEK, { rid, offset, whence });
}

/** Seek a file ID to the given offset under mode given by `whence`.
 *
 *      const file = await Deno.open("/foo/bar.txt");
 *      await Deno.seek(file.rid, 0, 0);
 */
export async function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<void> {
  await sendAsyncJson(dispatch.OP_SEEK, { rid, offset, whence });
}

/** Close the file ID. */
export function close(rid: number): void {
  sendSyncJson(dispatch.OP_CLOSE, { rid });
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

  seek(offset: number, whence: SeekMode): Promise<void> {
    return seek(this.rid, offset, whence);
  }

  seekSync(offset: number, whence: SeekMode): void {
    return seekSync(this.rid, offset, whence);
  }

  close(): void {
    close(this.rid);
  }
}

/** An instance of `File` for stdin. */
export const stdin = new File(0);
/** An instance of `File` for stdout. */
export const stdout = new File(1);
/** An instance of `File` for stderr. */
export const stderr = new File(2);

export interface OpenOptions {
  /** Sets the option for read access. This option, when true, will indicate that the file should be read-able if opened. */
  read?: boolean;
  /** Sets the option for write access.
   * This option, when true, will indicate that the file should be write-able if opened.
   * If the file already exists, any write calls on it will overwrite its contents, without truncating it.
   */
  write?: boolean;
  /** Sets the option for creating a new file.
   * This option indicates whether a new file will be created if the file does not yet already exist.
   * In order for the file to be created, write or append access must be used.
   */
  create?: boolean;
  /** Sets the option for truncating a previous file.
   * If a file is successfully opened with this option set it will truncate the file to 0 length if it already exists.
   * The file must be opened with write access for truncate to work.
   */
  truncate?: boolean;
  /**Sets the option for the append mode.
   * This option, when true, means that writes will append to a file instead of overwriting previous contents.
   * Note that setting { write: true, append: true } has the same effect as setting only { append: true }.
   */
  append?: boolean;
  /** Sets the option to always create a new file.
   * This option indicates whether a new file will be created. No file is allowed to exist at the target location, also no (dangling) symlink.
   * If { createNew: true } is set, create and truncate are ignored.
   */
  createNew?: boolean;
}

export type OpenMode =
  /** Read-only. Default. Starts at beginning of file. */
  | "r"
  /** Read-write. Start at beginning of file. */
  | "r+"
  /** Write-only. Opens and truncates existing file or creates new one for
   * writing only.
   */
  | "w"
  /** Read-write. Opens and truncates existing file or creates new one for
   * writing and reading.
   */
  | "w+"
  /** Write-only. Opens existing file or creates new one. Each write appends
   * content to the end of file.
   */
  | "a"
  /** Read-write. Behaves like "a" and allows to read from file. */
  | "a+"
  /** Write-only. Exclusive create - creates new file only if one doesn't exist
   * already.
   */
  | "x"
  /** Read-write. Behaves like `x` and allows to read from file. */
  | "x+";

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
