// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";
import { flatbuffers } from "flatbuffers";
import { maybeThrowError } from "./errors";
import { deno as fbs } from "gen/msg_generated";

// @internal
export function send(
  builder: flatbuffers.Builder,
  msgType: fbs.Any,
  msg: flatbuffers.Offset
): null | fbs.Base {
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, msgType);
  builder.finish(fbs.Base.endBase(builder));

  const resBuf = libdeno.send(builder.asUint8Array());
  if (resBuf == null) {
    return null;
  } else {
    const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
    const baseRes = fbs.Base.getRootAsBase(bb);
    maybeThrowError(baseRes);
    return baseRes;
  }
}
