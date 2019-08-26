// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal";
import * as json from "./dispatch_json";

// These consts are shared with Rust. Update with care.
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
export const OP_METRICS = 19;
export const OP_REPL_START = 20;
export const OP_REPL_READLINE = 21;
export const OP_ACCEPT = 22;
export const OP_DIAL = 23;
export const OP_SHUTDOWN = 24;
export const OP_LISTEN = 25;
export const OP_RESOURCES = 26;
export const OP_GET_RANDOM_VALUES = 27;
export const OP_GLOBAL_TIMER_STOP = 28;
export const OP_GLOBAL_TIMER = 29;
export const OP_NOW = 30;
export const OP_PERMISSIONS = 31;
export const OP_REVOKE_PERMISSION = 32;
export const OP_CREATE_WORKER = 33;
export const OP_HOST_GET_WORKER_CLOSED = 34;
export const OP_HOST_POST_MESSAGE = 35;
export const OP_HOST_GET_MESSAGE = 36;
export const OP_WORKER_POST_MESSAGE = 37;
export const OP_WORKER_GET_MESSAGE = 38;
export const OP_RUN = 39;
export const OP_RUN_STATUS = 40;
export const OP_KILL = 41;
export const OP_CHDIR = 42;
export const OP_MKDIR = 43;
export const OP_CHMOD = 44;
export const OP_CHOWN = 45;
export const OP_REMOVE = 46;
export const OP_COPY_FILE = 47;
export const OP_STAT = 48;
export const OP_READ_DIR = 49;
export const OP_RENAME = 50;
export const OP_LINK = 51;
export const OP_SYMLINK = 52;
export const OP_READ_LINK = 53;
export const OP_TRUNCATE = 54;
export const OP_MAKE_TEMP_DIR = 55;
export const OP_CWD = 56;

export function asyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  switch (opId) {
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
    case OP_REPL_START:
    case OP_REPL_READLINE:
    case OP_ACCEPT:
    case OP_DIAL:
    case OP_GLOBAL_TIMER:
    case OP_HOST_GET_WORKER_CLOSED:
    case OP_HOST_GET_MESSAGE:
    case OP_WORKER_GET_MESSAGE:
    case OP_RUN_STATUS:
    case OP_MKDIR:
    case OP_CHMOD:
    case OP_CHOWN:
    case OP_REMOVE:
    case OP_COPY_FILE:
    case OP_STAT:
    case OP_READ_DIR:
    case OP_RENAME:
    case OP_LINK:
    case OP_SYMLINK:
    case OP_READ_LINK:
    case OP_TRUNCATE:
    case OP_MAKE_TEMP_DIR:
      json.asyncMsgFromRust(opId, ui8);
      break;
    default:
      throw Error("bad async opId");
  }
}
