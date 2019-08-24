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
export const OP_SET_ENV = 8;
export const OP_HOME_DIR = 9;
export const OP_START = 10;
export const OP_APPLY_SOURCE_MAP = 11;
export const OP_FORMAT_ERROR = 12;
export const OP_CACHE = 13;
export const OP_FETCH_SOURCE_FILE = 14;
export const OP_OPEN = 15;
export const OP_CLOSE = 16;
export const OP_SEEK = 17;
export const OP_FETCH = 18;

export function asyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  switch (opId) {
    case OP_FLATBUFFER:
      flatbuffers.asyncMsgFromRust(opId, ui8);
      break;
    case OP_WRITE:
    case OP_READ:
      minimal.asyncMsgFromRust(opId, ui8);
      break;
    case OP_EXIT:
    case OP_IS_TTY:
    case OP_ENV:
    case OP_EXEC_PATH:
    case OP_UTIME:
    case OP_OPEN:
    case OP_SEEK:
    case OP_FETCH:
      json.asyncMsgFromRust(opId, ui8);
      break;
    default:
      throw Error("bad async opId");
  }
}
