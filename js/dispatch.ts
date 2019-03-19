// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import * as errors from "./errors";
import * as util from "./util";
import { window } from "./window";

const DISPATCH_MINIMAL = 0xcafe;

let nextPromiseId = 0;
const promiseTable = new Map<number, util.Resolvable<msg.Base>>();
const promiseTable2 = new Map<number, util.Resolvable<number>>();

interface Record {
  promiseId: number;
  opId: number;
  arg: number;
  result: number;
  base?: msg.Base;
}

function recordFromBuf(buf: Uint8Array): Record {
  // assert(buf.byteLength % 4 == 0);
  const buf32 = new Int32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
  if (buf32[0] == DISPATCH_MINIMAL) {
    return {
      promiseId: buf32[1],
      opId: buf32[2],
      arg: buf32[3],
      result: buf32[4]
    };
  } else {
    const bb = new flatbuffers.ByteBuffer(buf);
    const base = msg.Base.getRootAsBase(bb);
    const cmdId = base.cmdId();
    return {
      promiseId: cmdId,
      arg: -1,
      result: 0,
      opId: -1,
      base
    };
  }
}

export function handleAsyncMsgFromRust(ui8: Uint8Array): void {
  const record = recordFromBuf(ui8);

  if (record.base) {
    // Legacy
    const { promiseId, base } = record;
    const promise = promiseTable.get(promiseId);
    util.assert(promise != null, `Expecting promise in table. ${promiseId}`);
    promiseTable.delete(record.promiseId);
    const err = errors.maybeError(base);
    if (err != null) {
      promise!.reject(err);
    } else {
      promise!.resolve(base);
    }
  } else {
    // Fast and new
    util.log("minimal handleAsyncMsgFromRust ", ui8.length);
    const { promiseId, result } = record;
    const promise = promiseTable2.get(promiseId);
    promiseTable2.delete(promiseId);
    promise!.resolve(result);
  }
}

function ui8FromArrayBufferView(abv: ArrayBufferView): Uint8Array {
  return new Uint8Array(abv.buffer, abv.byteOffset, abv.byteLength);
}

function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  zeroCopy: undefined | ArrayBufferView,
  sync = true
): [number, null | Uint8Array] {
  const cmdId = nextPromiseId++;
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, sync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const control = builder.asUint8Array();

  //const response = DenoCore.dispatch(
  const response = window.DenoCore.dispatch(
    control,
    zeroCopy ? ui8FromArrayBufferView(zeroCopy!) : undefined
  );

  builder.inUse = false;
  return [cmdId, response];
}

// @internal
export function sendAsync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): Promise<msg.Base> {
  const [cmdId, response] = sendInternal(
    builder,
    innerType,
    inner,
    data,
    false
  );
  util.assert(response == null);
  const promise = util.createResolvable<msg.Base>();
  promiseTable.set(cmdId, promise);
  return promise;
}

const scratch32 = new Int32Array(5);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength
);
util.assert(scratchBytes.byteLength === scratch32.length * 4);

export function sendAsync2(
  opId: number,
  arg: number,
  zeroCopy: Uint8Array
): Promise<number> {
  const promiseId = nextPromiseId++; // AKA cmdId

  scratch32[0] = DISPATCH_MINIMAL;
  scratch32[1] = promiseId;
  scratch32[2] = opId;
  scratch32[3] = arg;
  // scratch32[4] = -1;

  const promise = util.createResolvable<number>();
  promiseTable2.set(promiseId, promise);

  window.DenoCore.dispatch(scratchBytes, zeroCopy);

  //DenoCore.dispatch(scratchBytes, zeroCopy);
  return promise;
}

// @internal
export function sendSync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): null | msg.Base {
  const [cmdId, response] = sendInternal(builder, innerType, inner, data, true);
  util.assert(cmdId >= 0);
  if (response == null || response.length === 0) {
    return null;
  } else {
    const bb = new flatbuffers.ByteBuffer(response);
    const baseRes = msg.Base.getRootAsBase(bb);
    errors.maybeThrowError(baseRes);
    return baseRes;
  }
}
