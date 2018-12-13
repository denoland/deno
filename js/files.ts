// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { Reader, Writer, Closer, ReadResult } from "./io";
import * as dispatch from "./dispatch";
import * as msg from "gen/msg_generated";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";

/** The Deno abstraction for reading and writing files. */
export class File implements Reader, Writer, Closer {
  constructor(readonly rid: number) {}

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  read(p: Uint8Array): Promise<ReadResult> {
    return read(this.rid, p);
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
 */
export function create(filename: string): Promise<File> {
  return open(filename, "w+");
}

/** Open a file and return an instance of the `File` object.
 *
 *       import * as deno from "deno";
 *       (async () => {
 *         const file = await deno.open("/foo/bar.txt");
 *       })();
 */
export async function open(
  filename: string,
  mode: OpenMode = "r"
): Promise<File> {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const mode_ = builder.createString(mode);
  msg.Open.startOpen(builder);
  msg.Open.addFilename(builder, filename_);
  msg.Open.addMode(builder, mode_);
  const inner = msg.Open.endOpen(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Open, inner);
  assert(baseRes != null);
  assert(msg.Any.OpenRes === baseRes!.innerType());
  const res = new msg.OpenRes();
  assert(baseRes!.inner(res) != null);
  const rid = res.rid();
  return new File(rid);
}

/** Read from a file ID into an array buffer.
 *
 * Resolves with the `ReadResult` for the operation.
 */
export async function read(rid: number, p: Uint8Array): Promise<ReadResult> {
  const builder = flatbuffers.createBuilder();
  msg.Read.startRead(builder);
  msg.Read.addRid(builder, rid);
  const inner = msg.Read.endRead(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Read, inner, p);
  assert(baseRes != null);
  assert(msg.Any.ReadRes === baseRes!.innerType());
  const res = new msg.ReadRes();
  assert(baseRes!.inner(res) != null);
  return { nread: res.nread(), eof: res.eof() };
}

/** Write to the file ID the contents of the array buffer.
 *
 * Resolves with the number of bytes written.
 */
export async function write(rid: number, p: Uint8Array): Promise<number> {
  const builder = flatbuffers.createBuilder();
  msg.Write.startWrite(builder);
  msg.Write.addRid(builder, rid);
  const inner = msg.Write.endWrite(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Write, inner, p);
  assert(baseRes != null);
  assert(msg.Any.WriteRes === baseRes!.innerType());
  const res = new msg.WriteRes();
  assert(baseRes!.inner(res) != null);
  return res.nbyte();
}

/** Close the file ID. */
export function close(rid: number): void {
  const builder = flatbuffers.createBuilder();
  msg.Close.startClose(builder);
  msg.Close.addRid(builder, rid);
  const inner = msg.Close.endClose(builder);
  dispatch.sendSync(builder, msg.Any.Close, inner);
}
