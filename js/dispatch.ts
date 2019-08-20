// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal";
import * as flatbuffers from "./dispatch_flatbuffers";

// These consts are shared with Rust. Update with care.
export const OP_FLATBUFFER = 44;
export const OP_READ = 1;
export const OP_WRITE = 2;

export function handleAsyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  switch (opId) {
    case OP_FLATBUFFER:
      flatbuffers.handleAsyncMsgFromRust(opId, ui8);
      break;
    case OP_WRITE:
    case OP_READ:
      minimal.handleAsyncMsgFromRust(opId, ui8);
      break;
    default:
      throw Error("bad opId");
  }
}
