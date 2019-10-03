// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal.ts";
import * as json from "./dispatch_json.ts";

// These consts are shared with Rust. Update with care.
export let OP_READ: number;
export let OP_WRITE: number;
export let OP_EXIT: number;
export let OP_IS_TTY: number;
export let OP_ENV: number;
export let OP_EXEC_PATH: number;
export let OP_UTIME: number;
export let OP_SET_ENV: number;
export let OP_GET_ENV: number;
export let OP_HOME_DIR: number;
export let OP_START: number;
export let OP_APPLY_SOURCE_MAP: number;
export let OP_FORMAT_ERROR: number;
export let OP_CACHE: number;
export let OP_FETCH_SOURCE_FILES: number;
export let OP_OPEN: number;
export let OP_CLOSE: number;
export let OP_SEEK: number;
export let OP_FETCH: number;
export let OP_METRICS: number;
export let OP_REPL_START: number;
export let OP_REPL_READLINE: number;
export let OP_ACCEPT: number;
export let OP_DIAL: number;
export let OP_SHUTDOWN: number;
export let OP_LISTEN: number;
export let OP_RESOURCES: number;
export let OP_GET_RANDOM_VALUES: number;
export let OP_GLOBAL_TIMER_STOP: number;
export let OP_GLOBAL_TIMER: number;
export let OP_NOW: number;
export let OP_PERMISSIONS: number;
export let OP_REVOKE_PERMISSION: number;
export let OP_CREATE_WORKER: number;
export let OP_HOST_GET_WORKER_CLOSED: number;
export let OP_HOST_POST_MESSAGE: number;
export let OP_HOST_GET_MESSAGE: number;
export let OP_WORKER_POST_MESSAGE: number;
export let OP_WORKER_GET_MESSAGE: number;
export let OP_RUN: number;
export let OP_RUN_STATUS: number;
export let OP_KILL: number;
export let OP_CHDIR: number;
export let OP_MKDIR: number;
export let OP_CHMOD: number;
export let OP_CHOWN: number;
export let OP_REMOVE: number;
export let OP_COPY_FILE: number;
export let OP_STAT: number;
export let OP_READ_DIR: number;
export let OP_RENAME: number;
export let OP_LINK: number;
export let OP_SYMLINK: number;
export let OP_READ_LINK: number;
export let OP_TRUNCATE: number;
export let OP_MAKE_TEMP_DIR: number;
export let OP_CWD: number;
export let OP_FETCH_ASSET: number;
export let OP_DIAL_TLS: number;
export let OP_HOSTNAME: number;

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
    case OP_DIAL_TLS:
    case OP_FETCH_SOURCE_FILES:
      json.asyncMsgFromRust(opId, ui8);
      break;
    default:
      throw Error("bad async opId");
  }
}
