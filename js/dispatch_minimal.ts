// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
import * as util from "./util";
import { core } from "./core";

const DISPATCH_MINIMAL_TOKEN = 0xcafe;
const promiseTableMin = new Map<number, util.Resolvable<number>>();

export interface RecordMinimal {
  opId: number;
  arg: number;
  result: number;
}

/** Determines if a message has the "minimal" serialization format. If false, it
 * is flatbuffer encoded.
 */
export function hasMinimalToken(i32: Int32Array): boolean {
  return i32[0] == DISPATCH_MINIMAL_TOKEN;
}

export function recordFromBufMinimal(buf32: Int32Array): null | RecordMinimal {
  if (hasMinimalToken(buf32)) {
    return {
      opId: buf32[1],
      arg: buf32[2],
      result: buf32[3]
    };
  }
  return null;
}

const scratch32 = new Int32Array(5);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength
);
util.assert(scratchBytes.byteLength === scratch32.length * 4);

export function handleAsyncMsgFromRustMinimal(
  promiseId: number,
  ui8: Uint8Array,
  record: RecordMinimal
): void {
  // Fast and new
  util.log("minimal handleAsyncMsgFromRust ", ui8.length);
  const { result } = record;
  const promise = promiseTableMin.get(promiseId);
  promiseTableMin.delete(promiseId);
  promise!.resolve(result);
}

export function sendAsyncMinimal(
  opId: number,
  arg: number,
  zeroCopy: Uint8Array
): Promise<number> {
  scratch32[0] = DISPATCH_MINIMAL_TOKEN;
  scratch32[1] = opId;
  scratch32[2] = arg;

  const promiseId = core.dispatch(scratchBytes, zeroCopy);

  util.assert(typeof promiseId == "number");

  const promise = util.createResolvable<number>();
  promiseTableMin.set(promiseId as number, promise);

  return promise;
}
