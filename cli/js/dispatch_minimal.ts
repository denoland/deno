// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as util from "./util.ts";
import { core } from "./core.ts";
import { TextDecoder } from "./text_encoding.ts";
import { errors, ErrorKind, constructError } from "./errors.ts";

const promiseTableMin = new Map<number, util.Resolvable<number>>();
// Note it's important that promiseId starts at 1 instead of 0, because sync
// messages are indicated with promiseId 0. If we ever add wrap around logic for
// overflows, this should be taken into account.
let _nextPromiseId = 1;

const decoder = new TextDecoder();

function nextPromiseId(): number {
  return _nextPromiseId++;
}

export interface RecordMinimal {
  promiseId: number;
  arg: number;
  result: number;
  err?: {
    kind: ErrorKind;
    message: string;
  };
}

const scratch32 = new Int32Array(3);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength
);
util.assert(scratchBytes.byteLength === scratch32.length * 4);

export function asyncMsgFromRust(ui8: Uint8Array): void {
  const buf32 = new Int32Array(ui8.buffer, ui8.byteOffset, 3);
  const promiseId = buf32[0];
  const promise = promiseTableMin.get(promiseId);
  promiseTableMin.delete(promiseId);
  util.assert(promise);
  const arg1 = buf32[1];
  const result = buf32[2];
  if (arg1 < 0) {
    const kind = result as ErrorKind;
    const message = decoder.decode(ui8.subarray(12));
    promise.reject(() => {
      constructError(kind, message);
    });
    return;
  } else if (ui8.length != 12) {
    promise.reject(() => {
      throw new errors.InvalidData("BadMessage");
    });
    return;
  }
  promise.resolve(result);
}

export async function sendAsyncMinimal(
  opId: number,
  arg: number,
  zeroCopy: Uint8Array
): Promise<number> {
  const promiseId = nextPromiseId(); // AKA cmdId
  scratch32[0] = promiseId;
  scratch32[1] = arg;
  scratch32[2] = 0; // result
  const promise = util.createResolvable<number>();
  core.dispatch(opId, scratchBytes, zeroCopy);
  // Async result.
  promiseTableMin.set(promiseId, promise);
  return await promise;
}

export function sendSyncMinimal(
  opId: number,
  arg: number,
  zeroCopy: Uint8Array
): number {
  scratch32[0] = 0; // promiseId 0 indicates sync
  scratch32[1] = arg;
  const ui8 = core.dispatch(opId, scratchBytes, zeroCopy)!;
  const buf32 = new Int32Array(ui8.buffer, ui8.byteOffset, 3);
  const arg1 = buf32[1];
  const result = buf32[2];
  if (arg1 < 0) {
    const kind = result as ErrorKind;
    const message = decoder.decode(ui8.subarray(12));
    constructError(kind, message);
  } else if (ui8.length != 12) {
    throw new errors.InvalidData("BadMessage");
  }
  return result;
}
