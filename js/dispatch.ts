// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import * as errors from "./errors";
import * as util from "./util";

let nextCmdId = 0;
const promiseTable = new Map<number, util.Resolvable<msg.Base>>();

export function handleAsyncMsgFromRust(ui8: Uint8Array): void {
  const bb = new flatbuffers.ByteBuffer(ui8);
  const base = msg.Base.getRootAsBase(bb);
  const cmdId = base.cmdId();
  const promise = promiseTable.get(cmdId);
  util.assert(promise != null, `Expecting promise in table. ${cmdId}`);
  promiseTable.delete(cmdId);
  const err = errors.maybeError(base);
  if (err != null) {
    promise!.reject(err);
  } else {
    promise!.resolve(base);
  }
}

function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  zeroCopy: undefined | ArrayBufferView,
  sync = true
): [number, null | Uint8Array] {
  const cmdId = nextCmdId++;
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, sync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const control = builder.asUint8Array();
  const response = window.DenoCore.dispatch(control, zeroCopy);

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
