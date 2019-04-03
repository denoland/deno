// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import {
  Reader,
  Writer,
  Seeker,
  Closer,
  ReadResult,
  SeekMode,
  SyncReader,
  SyncWriter,
  SyncSeeker
} from "./io";
import * as dispatch from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";

function reqOpen(
  filename: string,
  mode: OpenMode
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const mode_ = builder.createString(mode);
  msg.Open.startOpen(builder);
  msg.Open.addFilename(builder, filename_);
  msg.Open.addMode(builder, mode_);
  const inner = msg.Open.endOpen(builder);
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
  msg.Read.startRead(builder);
  msg.Read.addRid(builder, rid);
  const inner = msg.Read.endRead(builder);
  return [builder, msg.Any.Read, inner, p];
}

function resRead(baseRes: null | msg.Base): ReadResult {
  assert(baseRes != null);
  assert(msg.Any.ReadRes === baseRes!.innerType());
  const res = new msg.ReadRes();
  assert(baseRes!.inner(res) != null);
  return { nread: res.nread(), eof: res.eof() };
}

/** Read synchronously from a file ID into an array buffer.
 *
 * Return `ReadResult` for the operation.
 *
 *      const file = Deno.openSync("/foo/bar.txt");
 *      const buf = new Uint8Array(100);
 *      const { nread, eof } = Deno.readSync(file.rid, buf);
 *      const text = new TextDecoder.decode(buf);
 *
 */
export function readSync(rid: number, p: Uint8Array): ReadResult {
  return resRead(dispatch.sendSync(...reqRead(rid, p)));
}

/** Read from a file ID into an array buffer.
 *
 * Resolves with the `ReadResult` for the operation.
 *
 *       (async () => {
 *         const file = await Deno.open("/foo/bar.txt");
 *         const buf = new Uint8Array(100);
 *         const { nread, eof } = await Deno.read(file.rid, buf);
 *         const text = new TextDecoder.decode(buf);
 *       })();
 */
export async function read(rid: number, p: Uint8Array): Promise<ReadResult> {
  return resRead(await dispatch.sendAsync(...reqRead(rid, p)));
}

function reqWrite(
  rid: number,
  p: Uint8Array
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, Uint8Array] {
  const builder = flatbuffers.createBuilder();
  msg.Write.startWrite(builder);
  msg.Write.addRid(builder, rid);
  const inner = msg.Write.endWrite(builder);
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
  return resWrite(await dispatch.sendAsync(...reqWrite(rid, p)));
}

function reqSeek(
  rid: number,
  offset: number,
  whence: SeekMode
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  msg.Seek.startSeek(builder);
  msg.Seek.addRid(builder, rid);
  msg.Seek.addOffset(builder, offset);
  msg.Seek.addWhence(builder, whence);
  const inner = msg.Seek.endSeek(builder);
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
  msg.Close.startClose(builder);
  msg.Close.addRid(builder, rid);
  const inner = msg.Close.endClose(builder);
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

  read(p: Uint8Array): Promise<ReadResult> {
    return read(this.rid, p);
  }

  readSync(p: Uint8Array): ReadResult {
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
