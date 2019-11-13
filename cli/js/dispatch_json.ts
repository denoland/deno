// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as util from "./util.ts";
import { TextEncoder, TextDecoder } from "./text_encoding.ts";
import { core } from "./core.ts";
import { ErrorKind, DenoError } from "./errors.ts";

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

const promiseTable = new Map<number, util.Resolvable<JsonResponse>>();
let _nextPromiseId = 1;

function nextPromiseId(): number {
  return _nextPromiseId++;
}

function decode(ui8: Uint8Array): JsonResponse {
  const s = new TextDecoder().decode(ui8);
  return JSON.parse(s) as JsonResponse;
}

function encode(args: object): Uint8Array {
  const s = JSON.stringify(args);
  return new TextEncoder().encode(s);
}

function unwrapResponse(res: JsonResponse): Ok {
  if (res.err != null) {
    throw new DenoError(res.err!.kind, res.err!.message);
  }
  util.assert(res.ok != null);
  return res.ok;
}

export function asyncMsgFromRust(opId: number, resUi8: Uint8Array): void {
  const res = decode(resUi8);
  util.assert(res.promiseId != null);

  const promise = promiseTable.get(res.promiseId!);
  util.assert(promise != null);
  promiseTable.delete(res.promiseId!);
  promise.resolve(res);
}

export function sendSync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Ok {
  const argsUi8 = encode(args);
  const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
  util.assert(resUi8 != null);

  const res = decode(resUi8!);
  util.assert(res.promiseId == null);
  return unwrapResponse(res);
}

export async function sendAsync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Promise<Ok> {
  const promiseId = nextPromiseId();
  args = Object.assign(args, { promiseId });
  const promise = util.createResolvable<Ok>();

  const argsUi8 = encode(args);
  const buf = core.dispatch(opId, argsUi8, zeroCopy);
  if (buf) {
    // Sync result.
    const res = decode(buf);
    promise.resolve(res);
  } else {
    // Async result.
    promiseTable.set(promiseId, promise);
  }

  const res = await promise;
  return unwrapResponse(res);
}
