// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as util from "../util.ts";
import { core } from "../core.ts";
import { TextDecoder } from "../web/text_encoding.ts";
import { ErrorKind, errors, getErrorClass } from "../errors.ts";

// Using an object without a prototype because `Map` was causing GC problems.
const promiseTableMin: {
  [key: number]: util.Resolvable<RecordMinimal>;
} = Object.create(null);

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

export function recordFromBufMinimal(ui8: Uint8Array): RecordMinimal {
  const header = ui8.subarray(0, 12);
  const buf32 = new Int32Array(
    header.buffer,
    header.byteOffset,
    header.byteLength / 4
  );
  const promiseId = buf32[0];
  const arg = buf32[1];
  const result = buf32[2];
  let err;

  if (arg < 0) {
    const kind = result as ErrorKind;
    const message = decoder.decode(ui8.subarray(12));
    err = { kind, message };
  } else if (ui8.length != 12) {
    throw new errors.InvalidData("BadMessage");
  }

  return {
    promiseId,
    arg,
    result,
    err,
  };
}

function unwrapResponse(res: RecordMinimal): number {
  if (res.err != null) {
    throw new (getErrorClass(res.err.kind))(res.err.message);
  }
  return res.result;
}

const scratch32 = new Int32Array(3);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength
);
util.assert(scratchBytes.byteLength === scratch32.length * 4);

export function asyncMsgFromRust(ui8: Uint8Array): void {
  const record = recordFromBufMinimal(ui8);
  const { promiseId } = record;
  const promise = promiseTableMin[promiseId];
  delete promiseTableMin[promiseId];
  util.assert(promise);
  promise.resolve(record);
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
  const promise = util.createResolvable<RecordMinimal>();
  const buf = core.dispatch(opId, scratchBytes, zeroCopy);
  if (buf) {
    const record = recordFromBufMinimal(buf);
    // Sync result.
    promise.resolve(record);
  } else {
    // Async result.
    promiseTableMin[promiseId] = promise;
  }

  const res = await promise;
  return unwrapResponse(res);
}

export function sendSyncMinimal(
  opId: number,
  arg: number,
  zeroCopy: Uint8Array
): number {
  scratch32[0] = 0; // promiseId 0 indicates sync
  scratch32[1] = arg;
  const res = core.dispatch(opId, scratchBytes, zeroCopy)!;
  const resRecord = recordFromBufMinimal(res);
  return unwrapResponse(resRecord);
}
