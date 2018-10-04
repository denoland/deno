// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { Reader, Writer, Closer, ReadResult } from "./io";
import * as dispatch from "./dispatch";
import * as msg from "gen/msg_generated";
import { assert } from "./util";
import { flatbuffers } from "flatbuffers";

export class File implements Reader, Writer, Closer {
  constructor(readonly fd: number) {}

  write(p: ArrayBufferView): Promise<number> {
    return write(this.fd, p);
  }

  read(p: ArrayBufferView): Promise<ReadResult> {
    return read(this.fd, p);
  }

  close(): void {
    close(this.fd);
  }
}

export const stdin = new File(0);
export const stdout = new File(1);
export const stderr = new File(2);

// TODO This is just a placeholder - not final API.
export type OpenMode = "r" | "w" | "w+" | "x";

export function create(filename: string): Promise<File> {
  return open(filename, "x");
}

export async function open(
  filename: string,
  mode: OpenMode = "r"
): Promise<File> {
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  msg.Open.startOpen(builder);
  msg.Open.addFilename(builder, filename_);
  const inner = msg.Open.endOpen(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Open, inner);
  assert(baseRes != null);
  assert(msg.Any.OpenRes === baseRes!.innerType());
  const res = new msg.OpenRes();
  assert(baseRes!.inner(res) != null);
  const fd = res.rid();
  return new File(fd);
}

export async function read(
  fd: number,
  p: ArrayBufferView
): Promise<ReadResult> {
  const builder = new flatbuffers.Builder();
  msg.Read.startRead(builder);
  msg.Read.addRid(builder, fd);
  const inner = msg.Read.endRead(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Read, inner, p);
  assert(baseRes != null);
  assert(msg.Any.ReadRes === baseRes!.innerType());
  const res = new msg.ReadRes();
  assert(baseRes!.inner(res) != null);
  return { nread: res.nread(), eof: res.eof() };
}

export async function write(fd: number, p: ArrayBufferView): Promise<number> {
  const builder = new flatbuffers.Builder();
  msg.Write.startWrite(builder);
  msg.Write.addRid(builder, fd);
  const inner = msg.Write.endWrite(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Write, inner, p);
  assert(baseRes != null);
  assert(msg.Any.WriteRes === baseRes!.innerType());
  const res = new msg.WriteRes();
  assert(baseRes!.inner(res) != null);
  return res.nbyte();
}

export function close(fd: number): void {
  const builder = new flatbuffers.Builder();
  msg.Close.startClose(builder);
  msg.Close.addRid(builder, fd);
  const inner = msg.Close.endClose(builder);
  dispatch.sendSync(builder, msg.Any.Close, inner);
}
