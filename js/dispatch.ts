// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { core } from "./core";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/cli/msg_generated";
import * as errors from "./errors";
import * as util from "./util";
import {
  Record,
  nextPromiseId,
  recordFromBufMinimal,
  handleAsyncMsgFromRustMinimal
} from "./dispatch_minimal";

const promiseTable = new Map<number, util.Resolvable<msg.Base>>();

interface FlatbufferRecord extends Record {
  base?: msg.Base;
}

function recordFromBuf(buf: Uint8Array): FlatbufferRecord {
  // assert(buf.byteLength % 4 == 0);
  const buf32 = new Int32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);

  const recordMin = recordFromBufMinimal(buf32);
  if (recordMin) {
    return recordMin;
  } else {
    const bb = new flatbuffers.ByteBuffer(buf);
    const base = msg.Base.getRootAsBase(bb);
    const cmdId = base.cmdId();
    return {
      opId: -1,
      arg: -1,
      result: -1,
      promiseId: cmdId,
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
    handleAsyncMsgFromRustMinimal(ui8, record);
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
  const cmdId = nextPromiseId();
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, sync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const control = builder.asUint8Array();

  const response = core.dispatch(
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
