// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import * as errors from "./errors";
import * as util from "./util";

let nextCmdId = 0;
const promiseTable = new Map<number, util.Resolvable<msg.Base>>();

let fireTimers: () => void;

export function setFireTimersCallback(fn: () => void) {
  fireTimers = fn;
}

export function handleAsyncMsgFromRust2() {
  // This is the libdeno.setIdle callback
  util.log("handleAsyncMsgFromRust2");

  // TODO Should we loop here?

  // Before we block on beginReceive, we check if there are actually any pending
  // promises. If not, we return.
  if (promiseTable.size == 0) {
    util.log("promiseTable empty");
    return;
  }

  let { byteOffset, byteLength } = libdeno.rx.beginReceive();
  let buf = libdeno.rx.u8.subarray(byteOffset, byteOffset + byteLength);

  util.log("receive buf", buf.byteLength);
  handleAsyncMsgFromRust(buf);

  libdeno.rx.endReceive();
}

export function handleAsyncMsgFromRust(ui8: Uint8Array) {
  // If a the buffer is empty, recv() on the native side timed out and we
  // did not receive a message.
  if (ui8.length) {
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
  // Fire timers that have become runnable.
  fireTimers();
}

// @internal
export function sendAsync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): Promise<msg.Base> {
  const [cmdId, resBuf] = sendInternal(builder, innerType, inner, data, false);
  util.assert(resBuf == null);
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
  const [cmdId, resBuf] = sendInternal(builder, innerType, inner, data, true);
  util.assert(cmdId >= 0);
  if (resBuf == null) {
    return null;
  } else {
    const u8 = new Uint8Array(resBuf!);
    const bb = new flatbuffers.ByteBuffer(u8);
    const baseRes = msg.Base.getRootAsBase(bb);
    errors.maybeThrowError(baseRes);
    return baseRes;
  }
}

function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data: undefined | ArrayBufferView,
  sync = true
): [number, null | Uint8Array] {
  const cmdId = nextCmdId++;
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, sync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const u8 = builder.asUint8Array();
  util.log("sendInternal", sync, u8.byteLength, Array.from(u8));

  let res;
  if (sync) {
    res = libdeno.send(u8, data);
  } else {
    res = null;
  }

  libdeno.tx.resizeSend(u8.byteLength);
  libdeno.tx.endSend(!sync);

  // builder.inUse = false;
  return [cmdId, res];
}
