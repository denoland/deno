// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsyncMinimal, sendSyncMinimal } from "./dispatch_minimal.ts";
// TODO(bartlomieju): remove this import and maybe lazy-initialize
// OPS_CACHE that belongs only to this module
import { OPS_CACHE } from "../runtime.ts";

// This is done because read/write are extremely performance sensitive.
let OP_READ = -1;
let OP_WRITE = -1;

export function readSync(rid: number, buffer: Uint8Array): number | null {
  if (buffer.length == 0) {
    return 0;
  }
  if (OP_READ < 0) {
    OP_READ = OPS_CACHE["op_read"];
  }
  const nread = sendSyncMinimal(OP_READ, rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return null;
  } else {
    return nread;
  }
}

export async function read(
  rid: number,
  buffer: Uint8Array
): Promise<number | null> {
  if (buffer.length == 0) {
    return 0;
  }
  if (OP_READ < 0) {
    OP_READ = OPS_CACHE["op_read"];
  }
  const nread = await sendAsyncMinimal(OP_READ, rid, buffer);
  if (nread < 0) {
    throw new Error("read error");
  } else if (nread == 0) {
    return null;
  } else {
    return nread;
  }
}

export function writeSync(rid: number, data: Uint8Array): number {
  if (OP_WRITE < 0) {
    OP_WRITE = OPS_CACHE["op_write"];
  }
  const result = sendSyncMinimal(OP_WRITE, rid, data);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}

export async function write(rid: number, data: Uint8Array): Promise<number> {
  if (OP_WRITE < 0) {
    OP_WRITE = OPS_CACHE["op_write"];
  }
  const result = await sendAsyncMinimal(OP_WRITE, rid, data);
  if (result < 0) {
    throw new Error("write error");
  } else {
    return result;
  }
}
