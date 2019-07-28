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
} from "./io";
import * as dispatch from "./dispatch";
import { sendAsyncMinimal } from "./dispatch_minimal";
import * as msg from "gen/cli/msg_generated";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";

const OP_READ = 1;
const OP_WRITE = 2;

function reqOpen(
  filename: string,
  mode: OpenMode
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const mode_ = builder.createString(mode);
  const inner = msg.Open.createOpen(builder, filename_, 0, mode_);
  return [builder, msg.Any.Open, inner];
}

function resOpen(baseRes: null | msg.Base): File {
  assert(baseRes != null);
  assert(msg.Any.OpenRes === baseRes!.innerType());
  const res = new msg.OpenRes();
  assert(baseRes!.inner(res) != null);
  const rid = res.rid();
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  return new File(rid);
}

/** Open a file and return an instance of the `File` object
 *  synchronously.
 *
 *       const file = Deno.openSync("/foo/bar.txt");
 */
export function openSync(filename: string, mode: OpenMode = "r"): File {
  return resOpen(dispatch.sendSync(...reqOpen(filename, mode)));
}

/** Open a file and return an instance of the `File` object.
 *
 *       (async () => {
 *         const file = await Deno.open("/foo/bar.txt");
 *       })();
 */
export async function open(
  filename: string,
  mode: OpenMode = "r"
): Promise<File> {
  return resOpen(await dispatch.sendAsync(...reqOpen(filename, mode)));
}

function reqRead(
  rid: number,
  p: Uint8Array
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, Uint8Array] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Read.createRead(builder, rid);
  return [builder, msg.Any.Read, inner, p];
}

function resRead(baseRes: null | msg.Base): number | EOF {
  assert(baseRes != null);
  assert(msg.Any.ReadRes === baseRes!.innerType());
  const res = new msg.ReadRes();
  assert(baseRes!.inner(res) != null);
  if (res.eof()) {
    return EOF;
  }
  return res.nread();
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
  return resRead(dispatch.sendSync(...reqRead(rid, p)));
}

/** Read from a file ID into an array buffer.
 *
 * Resolves with the `number | EOF` for the operation.
 *
 *       (async () => {
 *         const file = await Deno.open("/foo/bar.txt");
 *         const buf = new Uint8Array(100);
 *         const nread = await Deno.read(file.rid, buf);
 *         const text = new TextDecoder().decode(buf);
 *       })();
 */
export async function read(rid: number, p: Uint8Array): Promise<number | EOF> {
  const nread = await sendAsyncMinimal(OP_READ, rid, p);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return EOF;
  } else {
    return nread;
  }
}

function reqWrite(
  rid: number,
  p: Uint8Array
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, Uint8Array] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Write.createWrite(builder, rid);
  return [builder, msg.Any.Write, inner, p];
}

function resWrite(baseRes: null | msg.Base): number {
  assert(baseRes != null);
  assert(msg.Any.WriteRes === baseRes!.innerType());
  const res = new msg.WriteRes();
  assert(baseRes!.inner(res) != null);
  return res.nbyte();
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
  return resWrite(dispatch.sendSync(...reqWrite(rid, p)));
}

/** Write to the file ID the contents of the array buffer.
 *
 * Resolves with the number of bytes written.
 *
 *      (async () => {
 *        const encoder = new TextEncoder();
 *        const data = encoder.encode("Hello world\n");
 *        const file = await Deno.open("/foo/bar.txt");
 *        await Deno.write(file.rid, data);
 *      })();
 *
 */
export async function write(rid: number, p: Uint8Array): Promise<number> {
  let result = await sendAsyncMinimal(OP_WRITE, rid, p);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

function reqSeek(
  rid: number,
  offset: number,
  whence: SeekMode
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Seek.createSeek(builder, rid, offset, whence);
  return [builder, msg.Any.Seek, inner];
}

/** Seek a file ID synchronously to the given offset under mode given by `whence`.
 *
 *       const file = Deno.openSync("/foo/bar.txt");
 *       Deno.seekSync(file.rid, 0, 0);
 */
export function seekSync(rid: number, offset: number, whence: SeekMode): void {
  dispatch.sendSync(...reqSeek(rid, offset, whence));
}

/** Seek a file ID to the given offset under mode given by `whence`.
 *
 *      (async () => {
 *        const file = await Deno.open("/foo/bar.txt");
 *        await Deno.seek(file.rid, 0, 0);
 *      })();
 */
export async function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<void> {
  await dispatch.sendAsync(...reqSeek(rid, offset, whence));
}

/** Close the file ID. */
export function close(rid: number): void {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Close.createClose(builder, rid);
  dispatch.sendSync(builder, msg.Any.Close, inner);
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
