// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
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
 *       const file = Deno.openSync("/foo/bar.txt");
 */
export function openSync(filename: string, mode: OpenMode = "r"): File {
  const rid = sendSyncJson(dispatch.OP_OPEN, { filename, mode });
  return new File(rid);
}

/** Open a file and return an instance of the `File` object.
 *
 *       const file = await Deno.open("/foo/bar.txt");
 */
export async function open(
  filename: string,
  mode: OpenMode = "r"
): Promise<File> {
  const rid = await sendAsyncJson(dispatch.OP_OPEN, { filename, mode });
  return new File(rid);
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

/** A factory function for creating instances of `File` associated with the
 * supplied file name.
 * @internal
 */
export function create(filename: string): Promise<File> {
  return open(filename, "w+");
}
