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

export function handleAsyncMsgFromRust(opId: number, resUi8: Uint8Array): void {
  const resStr = new TextDecoder().decode(resUi8);
  const res = JSON.parse(resStr) as JsonResponse;
  const promiseId = res.promiseId!;
  const promise = promiseTable.get(promiseId)!;
  if (!promise) {
    throw Error(`Async op ${opId} had bad promiseId: ${resStr}`);
  }
  promiseTable.delete(promiseId);

  if (res.err) {
    let err = maybeError(res.err);
    if (err) {
      promise.reject(err);
    } else {
      promise.resolve();
    }
  } else {
    promise.resolve(res.ok!);
  }
}

export function sendSync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Ok {
  const argsStr = JSON.stringify(args);
  const argsUi8 = new TextEncoder().encode(argsStr);
  const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
  if (!resUi8) {
    return;
  }
  const resStr = new TextDecoder().decode(resUi8);
  const res = JSON.parse(resStr) as JsonResponse;
  util.assert(!res.promiseId);
  if (res.err) {
    const err = maybeError(res.err);
    if (err != null) {
      throw err;
    }
  }
  return res.ok;
}

export function sendAsync(
  opId: number,
  args: object = {},
  zeroCopy?: Uint8Array
): Promise<Ok> {
  const promiseId = nextPromiseId();
  args = Object.assign(args, { promiseId });
  const argsStr = JSON.stringify(args);
  const argsUi8 = new TextEncoder().encode(argsStr);
  const promise = util.createResolvable<Ok>();
  promiseTable.set(promiseId, promise);
  const r = core.dispatch(opId, argsUi8, zeroCopy);
  util.assert(!r);
  return promise;
}

function maybeError(err: JsonError): null | DenoError<ErrorKind> {
  if (err.kind === ErrorKind.NoError) {
    return null;
  } else {
    return new DenoError(err.kind, err.message);
  }
}
