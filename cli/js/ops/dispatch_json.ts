// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as util from "../util.ts";
import { core } from "../core.ts";
import { OPS_CACHE } from "../runtime.ts";
import { ErrorKind, getErrorClass } from "../errors.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Ok = any;

interface JsonError {
  kind: ErrorKind;
  message: string;
}

interface JsonResponse {
  ok?: Ok;
  err?: JsonError;
  promiseId?: number; // Only present in async messages.
}

// Using an object without a prototype because `Map` was causing GC problems.
const promiseTable: {
  [key: number]: util.Resolvable<JsonResponse>;
} = Object.create(null);
let _nextPromiseId = 1;

function nextPromiseId(): number {
  return _nextPromiseId++;
}

function decode(ui8: Uint8Array): JsonResponse {
  const s = core.decode(ui8);
  return JSON.parse(s) as JsonResponse;
}

function encode(args: object): Uint8Array {
  const s = JSON.stringify(args);
  return core.encode(s);
}

function unwrapResponse(res: JsonResponse): Ok {
  if (res.err != null) {
    throw new (getErrorClass(res.err.kind))(res.err.message);
  }
  util.assert(res.ok != null);
  return res.ok;
}

export function asyncMsgFromRust(resUi8: Uint8Array): void {
  const res = decode(resUi8);
  util.assert(res.promiseId != null);

  const promise = promiseTable[res.promiseId!];
  util.assert(promise != null);
  delete promiseTable[res.promiseId!];
  promise.resolve(res);
}

export function sendSync(
  opName: string,
  args: object = {},
  ...zeroCopy: Uint8Array[]
): Ok {
  const opId = OPS_CACHE[opName];
  util.log("sendSync", opName, opId);
  const argsUi8 = encode(args);
  const resUi8 = core.dispatch(opId, argsUi8, ...zeroCopy);
  util.assert(resUi8 != null);

  const res = decode(resUi8);
  util.assert(res.promiseId == null);
  return unwrapResponse(res);
}

export async function sendAsync(
  opName: string,
  args: object = {},
  ...zeroCopy: Uint8Array[]
): Promise<Ok> {
  const opId = OPS_CACHE[opName];
  util.log("sendAsync", opName, opId);
  const promiseId = nextPromiseId();
  args = Object.assign(args, { promiseId });
  const promise = util.createResolvable<Ok>();

  const argsUi8 = encode(args);
  const buf = core.dispatch(opId, argsUi8, ...zeroCopy);
  if (buf) {
    // Sync result.
    const res = decode(buf);
    promise.resolve(res);
  } else {
    // Async result.
    promiseTable[promiseId] = promise;
  }

  const res = await promise;
  return unwrapResponse(res);
}
