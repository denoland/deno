// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsyncMinimal, sendSyncMinimal } from "./dispatch_minimal.ts";

export function readSync(rid: number, buffer: Uint8Array): number | null {
  if (buffer.length === 0) {
    return 0;
  }

  const nread = sendSyncMinimal("op_read", rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  }

  return nread === 0 ? null : nread;
}

export async function read(
  rid: number,
  buffer: Uint8Array,
): Promise<number | null> {
  if (buffer.length === 0) {
    return 0;
  }

  const nread = await sendAsyncMinimal("op_read", rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  }

  return nread === 0 ? null : nread;
}

export function writeSync(rid: number, data: Uint8Array): number {
  const result = sendSyncMinimal("op_write", rid, data);
  if (result < 0) {
    throw new Error("write error");
  }

  return result;
}

export async function write(rid: number, data: Uint8Array): Promise<number> {
  const result = await sendAsyncMinimal("op_write", rid, data);
  if (result < 0) {
    throw new Error("write error");
  }

  return result;
}
