// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import * as errors from "./errors";
import * as util from "./util";
import * as msgRing from "./msg_ring";

let nextCmdId = 0;
const promiseTable = new Map<number, util.Resolvable<msg.Base>>();

export function handleAsyncMsgFromRust(_unused: Uint8Array): void {
  util.log("handleAsyncMsgFromRust start");
  util.assert(_unused == null);
  let i = 0;
  let ui8: Uint8Array | null;
  while ((ui8 = msgRing.rx.receive(Uint8Array)) !== null) {
    util.log(`handleAsyncMsgFromRust ${i++}`);
    const bb = new flatbuffers.ByteBuffer(ui8!);
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
}

function sendInternal(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data: undefined | ArrayBufferView,
  sync = true
): number {
  const cmdId = nextCmdId++;
  msg.Base.startBase(builder);
  msg.Base.addInner(builder, inner);
  msg.Base.addInnerType(builder, innerType);
  msg.Base.addSync(builder, sync);
  msg.Base.addCmdId(builder, cmdId);
  builder.finish(msg.Base.endBase(builder));

  const ui8 = builder.asUint8Array();
  msgRing.tx.send(ui8);

  // Somehow put this in the shared buffer.

  libdeno.send(null, data);
  builder.inUse = false;
  return cmdId;
}

// @internal
export function sendAsync(
  builder: flatbuffers.Builder,
  innerType: msg.Any,
  inner: flatbuffers.Offset,
  data?: ArrayBufferView
): Promise<msg.Base> {
  const cmdId = sendInternal(builder, innerType, inner, data, false);
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
  const cmdId = sendInternal(builder, innerType, inner, data, true);
  util.assert(cmdId >= 0);
  let resBuf: Uint8Array | null = msgRing.rx.receive(Uint8Array);
  if (resBuf == null) {
    return null;
  } else {
    const bb = new flatbuffers.ByteBuffer(resBuf);
    const baseRes = msg.Base.getRootAsBase(bb);
    errors.maybeThrowError(baseRes);
    return baseRes;
  }
}
