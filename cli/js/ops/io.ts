// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsyncMinimal, sendSyncMinimal } from "./dispatch_minimal.ts";
import { EOF } from "../io.ts";
// TODO(bartlomieju): remove this import and maybe lazy-initialize
// OPS_CACHE that belongs only to this module
import { OPS_CACHE } from "../runtime.ts";

// This is done because read/write are extremely performance sensitive.
let OP_READ = -1;
let OP_WRITE = -1;

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
