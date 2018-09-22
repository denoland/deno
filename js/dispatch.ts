// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";
import { flatbuffers } from "flatbuffers";
import * as fbs from "gen/msg_generated";
import * as errors from "./errors";
import * as util from "./util";
import { maybePushTrace } from "./trace";

let nextCmdId = 0;
const promiseTable = new Map<number, util.Resolvable<fbs.Base>>();

export function handleAsyncMsgFromRust(ui8: Uint8Array) {
  const bb = new flatbuffers.ByteBuffer(ui8);
  const base = fbs.Base.getRootAsBase(bb);
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

// @internal
export function sendAsync(
  builder: flatbuffers.Builder,
  msgType: fbs.Any,
  msg: flatbuffers.Offset
): Promise<fbs.Base> {
  maybePushTrace(msgType, false); // add to trace if tracing
  const [cmdId, resBuf] = sendInternal(builder, msgType, msg, false);
  util.assert(resBuf == null);
  const promise = util.createResolvable<fbs.Base>();
  promiseTable.set(cmdId, promise);
  return promise;
}

// @internal
export function sendSync(
  builder: flatbuffers.Builder,
  msgType: fbs.Any,
  msg: flatbuffers.Offset
): null | fbs.Base {
  maybePushTrace(msgType, true); // add to trace if tracing
  const [cmdId, resBuf] = sendInternal(builder, msgType, msg, true);
  util.assert(cmdId >= 0);
  if (resBuf == null) {
    return null;
  } else {
    const u8 = new Uint8Array(resBuf!);
    // console.log("recv sync message", util.hexdump(u8));
    const bb = new flatbuffers.ByteBuffer(u8);
    const baseRes = fbs.Base.getRootAsBase(bb);
    errors.maybeThrowError(baseRes);
    return baseRes;
  }
}

function sendInternal(
  builder: flatbuffers.Builder,
  msgType: fbs.Any,
  msg: flatbuffers.Offset,
  sync = true
): [number, null | Uint8Array] {
  const cmdId = nextCmdId++;
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, msgType);
  fbs.Base.addSync(builder, sync);
  fbs.Base.addCmdId(builder, cmdId);
  builder.finish(fbs.Base.endBase(builder));

  return [cmdId, libdeno.send(builder.asUint8Array())];
}
