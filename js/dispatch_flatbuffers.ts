// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as flatbuffers from "./flatbuffers";
import { DenoError } from "./errors";
import { core } from "./core";
import * as msg from "gen/cli/msg_generated";
import * as util from "./util";
import { OP_FLATBUFFER } from "./dispatch";
export { msg, flatbuffers };

const promiseTable = new Map<number, util.Resolvable<msg.Base>>();
let _nextPromiseId = 1;

export function nextPromiseId(): number {
  return _nextPromiseId++;
}

interface FlatbufferRecord {
  promiseId: number;
  base: msg.Base;
}

export function asyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  let { promiseId, base } = flatbufferRecordFromBuf(ui8);
  const promise = promiseTable.get(promiseId);
  util.assert(promise != null, `Expecting promise in table. ${promiseId}`);
  promiseTable.delete(promiseId);
  const err = maybeError(base);
  if (err != null) {
    promise!.reject(err);
  } else {
    promise!.resolve(base);
  }
}

function flatbufferRecordFromBuf(buf: Uint8Array): FlatbufferRecord {
  const bb = new flatbuffers.ByteBuffer(buf);
  const base = msg.Base.getRootAsBase(bb);
  return {
    promiseId: base.cmdId(),
    base
  };
}

function ui8FromArrayBufferView(abv: ArrayBufferView): Uint8Array {
  return new Uint8Array(abv.buffer, abv.byteOffset, abv.byteLength);
}

function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  zeroCopy: undefined | ArrayBufferView,
  isSync: true
): Uint8Array;
function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  zeroCopy: undefined | ArrayBufferView,
  isSync: false
): Promise<msg.Base>;
function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  zeroCopy: undefined | ArrayBufferView,
  isSync: boolean
): Promise<msg.Base> | Uint8Array {
  const cmdId = nextPromiseId();
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, isSync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const control = builder.asUint8Array();

  const response = core.dispatch(
    OP_FLATBUFFER, // TODO(ry) Use actual opId later.
    control,
    zeroCopy ? ui8FromArrayBufferView(zeroCopy) : undefined
  );

  builder.inUse = false;

  if (response == null) {
    util.assert(!isSync);
    const promise = util.createResolvable<msg.Base>();
    promiseTable.set(cmdId, promise);
    return promise;
  } else {
    if (!isSync) {
      // We can easily and correctly allow for sync responses to async calls
      // by creating and returning a promise from the sync response.
      const bb = new flatbuffers.ByteBuffer(response);
      const base = msg.Base.getRootAsBase(bb);
      const err = maybeError(base);
      if (err != null) {
        return Promise.reject(err);
      } else {
        return Promise.resolve(base);
      }
    }
    return response;
  }
}

// @internal
export function sendAsync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): Promise<msg.Base> {
  return sendInternal(builder, innerType, inner, data, false);
}

// @internal
export function sendSync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): null | msg.Base {
  const response = sendInternal(builder, innerType, inner, data, true);
  if (response!.length === 0) {
    return null;
  } else {
    const bb = new flatbuffers.ByteBuffer(response!);
    const baseRes = msg.Base.getRootAsBase(bb);
    maybeThrowError(baseRes);
    return baseRes;
  }
}

function maybeError(base: msg.Base): null | DenoError<msg.ErrorKind> {
  const kind = base.errorKind();
  if (kind === msg.ErrorKind.NoError) {
    return null;
  } else {
    return new DenoError(kind, base.error()!);
  }
}

function maybeThrowError(base: msg.Base): void {
  const err = maybeError(base);
  if (err != null) {
    throw err;
  }
}
