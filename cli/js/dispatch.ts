// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal.ts";
import * as json from "./dispatch_json.ts";
import { AsyncHandler } from "./plugins.ts";

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
export let OP_GET_DIR: number;
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
export let OP_ACCEPT_TLS: number;
export let OP_DIAL: number;
export let OP_SHUTDOWN: number;
export let OP_LISTEN: number;
export let OP_LISTEN_TLS: number;
export let OP_RESOURCES: number;
export let OP_GET_RANDOM_VALUES: number;
export let OP_GLOBAL_TIMER_STOP: number;
export let OP_GLOBAL_TIMER: number;
export let OP_NOW: number;
export let OP_QUERY_PERMISSION: number;
export let OP_REVOKE_PERMISSION: number;
export let OP_REQUEST_PERMISSION: number;
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
export let OP_REALPATH: number;
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
export let OP_OPEN_PLUGIN: number;

type Action = (opId: number, ui8: Uint8Array) => void;

interface OPActionMap {
  [opID: number]: Action;
}

const PLUGIN_ASYNC_HANDLER_MAP: Map<number, AsyncHandler> = new Map();

export function setPluginAsyncHandler(
  opId: number,
  handler: AsyncHandler
): void {
  PLUGIN_ASYNC_HANDLER_MAP.set(opId, handler);
}

function getAction(opId: number): Action | void {
  const OP_ACTION_MAP: OPActionMap = {
    [OP_WRITE]: minimal.asyncMsgFromRust,
    [OP_READ]: minimal.asyncMsgFromRust,

    [OP_GET_DIR]: json.asyncMsgFromRust,
    [OP_EXIT]: json.asyncMsgFromRust,
    [OP_IS_TTY]: json.asyncMsgFromRust,
    [OP_ENV]: json.asyncMsgFromRust,
    [OP_EXEC_PATH]: json.asyncMsgFromRust,
    [OP_UTIME]: json.asyncMsgFromRust,
    [OP_OPEN]: json.asyncMsgFromRust,
    [OP_SEEK]: json.asyncMsgFromRust,
    [OP_FETCH]: json.asyncMsgFromRust,
    [OP_REPL_START]: json.asyncMsgFromRust,
    [OP_REPL_READLINE]: json.asyncMsgFromRust,
    [OP_ACCEPT]: json.asyncMsgFromRust,
    [OP_ACCEPT_TLS]: json.asyncMsgFromRust,
    [OP_DIAL]: json.asyncMsgFromRust,
    [OP_DIAL_TLS]: json.asyncMsgFromRust,
    [OP_GLOBAL_TIMER]: json.asyncMsgFromRust,
    [OP_HOST_GET_WORKER_CLOSED]: json.asyncMsgFromRust,
    [OP_HOST_GET_MESSAGE]: json.asyncMsgFromRust,
    [OP_WORKER_GET_MESSAGE]: json.asyncMsgFromRust,
    [OP_RUN_STATUS]: json.asyncMsgFromRust,
    [OP_MKDIR]: json.asyncMsgFromRust,
    [OP_CHMOD]: json.asyncMsgFromRust,
    [OP_CHOWN]: json.asyncMsgFromRust,
    [OP_REMOVE]: json.asyncMsgFromRust,
    [OP_COPY_FILE]: json.asyncMsgFromRust,
    [OP_STAT]: json.asyncMsgFromRust,
    [OP_REALPATH]: json.asyncMsgFromRust,
    [OP_READ_DIR]: json.asyncMsgFromRust,
    [OP_RENAME]: json.asyncMsgFromRust,
    [OP_LINK]: json.asyncMsgFromRust,
    [OP_SYMLINK]: json.asyncMsgFromRust,
    [OP_READ_LINK]: json.asyncMsgFromRust,
    [OP_TRUNCATE]: json.asyncMsgFromRust,
    [OP_MAKE_TEMP_DIR]: json.asyncMsgFromRust,
    [OP_FETCH_SOURCE_FILES]: json.asyncMsgFromRust
  };

  return OP_ACTION_MAP[opId];
}

export function asyncMsgFromRust(opId: number, ui8: Uint8Array): void {
  const action = getAction(opId);

  if (action) {
    action(opId, ui8);
  } else {
    const handler = PLUGIN_ASYNC_HANDLER_MAP.get(opId);
    if (handler) {
      handler(ui8);
    } else {
      throw Error("bad async opId");
    }
  }
}
