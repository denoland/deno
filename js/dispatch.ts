// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { core } from "./core";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/cli/msg_generated";
import * as errors from "./errors";
import * as util from "./util";
import {
  recordFromBufMinimal,
  handleAsyncMsgFromRustMinimal
} from "./dispatch_minimal";

const promiseTable = new Map<number, util.Resolvable<msg.Base>>();

function flatbufferRecordFromBuf(buf: Uint8Array): msg.Base {
  const bb = new flatbuffers.ByteBuffer(buf);
  const base = msg.Base.getRootAsBase(bb);
  return base;
}

export function handleAsyncMsgFromRust(
  promiseId: number,
  ui8: Uint8Array
): void {
  const buf32 = new Int32Array(ui8.buffer, ui8.byteOffset, ui8.byteLength / 4);
  const recordMin = recordFromBufMinimal(buf32);
  if (recordMin) {
    // Fast and new
    handleAsyncMsgFromRustMinimal(promiseId, ui8, recordMin);
  } else {
    // Legacy
    let base = flatbufferRecordFromBuf(ui8);
    const promise = promiseTable.get(promiseId);
    util.assert(promise != null, `Expecting promise in table. ${promiseId}`);
    promiseTable.delete(promiseId);
    const err = errors.maybeError(base);
    if (err != null) {
      promise!.reject(err);
    } else {
      promise!.resolve(base);
    }
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
  isSync: true
): Uint8Array | null;
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
): Promise<msg.Base> | Uint8Array | null {
  msg.Base.startBase(builder);
  msg.Base.addSync(builder, isSync);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  builder.finish(msg.Base.endBase(builder));

  const control = builder.asUint8Array();

  const response = core.dispatch(
    control,
    zeroCopy ? ui8FromArrayBufferView(zeroCopy) : undefined
  );

  builder.inUse = false;

  if (typeof response === "number") {
    const promise = util.createResolvable<msg.Base>();
    promiseTable.set(response, promise);
    util.assert(!isSync);
    return promise;
  } else {
    if (!isSync) {
      util.assert(response !== null);
      const base = flatbufferRecordFromBuf(response as Uint8Array);
      const err = errors.maybeError(base);
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
  const promise = sendInternal(builder, innerType, inner, data, false);
  return promise;
}

// @internal
export function sendSync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): null | msg.Base {
  const response = sendInternal(builder, innerType, inner, data, true);
  if (response == null || response.length === 0) {
    return null;
  } else {
    const bb = new flatbuffers.ByteBuffer(response!);
    const baseRes = msg.Base.getRootAsBase(bb);
    errors.maybeThrowError(baseRes);
    return baseRes;
  }
}
