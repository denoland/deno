// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal";
import * as flatbuffers from "./dispatch_flatbuffers";
import * as json from "./dispatch_json";

// These consts are shared with Rust. Update with care.
export const OP_FLATBUFFER = 44;
export const OP_READ = 1;
export const OP_WRITE = 2;
export const OP_EXIT = 3;
export const OP_IS_TTY = 4;
export const OP_ENV = 5;
export const OP_EXEC_PATH = 6;
export const OP_UTIME = 7;

export function handleAsyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  switch (opId) {
    case OP_FLATBUFFER:
      flatbuffers.handleAsyncMsgFromRust(opId, ui8);
      break;
    case OP_WRITE:
    case OP_READ:
      minimal.handleAsyncMsgFromRust(opId, ui8);
      break;
    case OP_EXIT:
    case OP_IS_TTY:
    case OP_ENV:
    case OP_EXEC_PATH:
    case OP_UTIME:
      json.handleAsyncMsgFromRust(opId, ui8);
      break;
    default:
      throw Error("bad opId");
  }
}
