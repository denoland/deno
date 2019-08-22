// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
// TODO(ry) Currently ErrorKind enum is defined in FlatBuffers. Therefore
// we must still reference the msg_generated.ts. This should be removed!
import { ErrorKind } from "gen/cli/msg_generated";
import * as util from "./util";
import { TextEncoder, TextDecoder } from "./text_encoding";
import { core } from "./core";
import { DenoError } from "./errors";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Ok = any;

interface JsonError {
  kind: ErrorKind;
  message: string;
}

interface JsonResponse {
  ok?: Ok;
  err?: JsonError;
  promiseId?: number; // only present in async mesasges.
}

const promiseTable = new Map<number, util.Resolvable<number>>();
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

function toDenoError(err: JsonError): DenoError<ErrorKind> {
  return new DenoError(err.kind, err.message);
}

export function asyncMsgFromRust(opId: number, res: Uint8Array): void {
  const { ok, err, promiseId } = decode(res);
  const promise = promiseTable.get(promiseId!)!;
  if (!promise) {
    throw Error(`Async op ${opId} had bad promiseId`);
  }
  promiseTable.delete(promiseId!);

  if (err) {
    promise.reject(toDenoError(err));
  } else if (ok) {
    promise.resolve(ok);
  } else {
    util.unreachable();
  }
}

export function sendSync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Ok {
  const argsUi8 = encode(args);
  const res = core.dispatch(opId, argsUi8, zeroCopy);
  if (!res) {
    return;
  }
  const { ok, err, promiseId } = decode(res);
  util.assert(!promiseId);
  if (err) {
    throw toDenoError(err);
  }
  return ok;
}

export function sendAsync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Promise<Ok> {
  const promiseId = nextPromiseId();
  args = Object.assign(args, { promiseId });
  const argsUi8 = encode(args);
  const promise = util.createResolvable<Ok>();
  promiseTable.set(promiseId, promise);
  const r = core.dispatch(opId, argsUi8, zeroCopy);
  util.assert(!r);
  return promise;
}
