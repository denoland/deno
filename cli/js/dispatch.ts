// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal.ts";
import * as json from "./dispatch_json.ts";
import { AsyncHandler } from "./plugins.ts";

export const OPS = {
  OP_OPS_MAP: undefined,
  OP_READ: undefined,
  OP_WRITE: undefined,
  OP_EXIT: undefined,
  OP_IS_TTY: undefined,
  OP_ENV: undefined,
  OP_EXEC_PATH: undefined,
  OP_UTIME: undefined,
  OP_SET_ENV: undefined,
  OP_GET_ENV: undefined,
  OP_GET_DIR: undefined,
  OP_START: undefined,
  OP_APPLY_SOURCE_MAP: undefined,
  OP_FORMAT_ERROR: undefined,
  OP_CACHE: undefined,
  OP_RESOLVE_MODULES: undefined,
  OP_FETCH_SOURCE_FILES: undefined,
  OP_OPEN: undefined,
  OP_CLOSE: undefined,
  OP_SEEK: undefined,
  OP_FETCH: undefined,
  OP_METRICS: undefined,
  OP_REPL_START: undefined,
  OP_REPL_READLINE: undefined,
  OP_ACCEPT: undefined,
  OP_ACCEPT_TLS: undefined,
  OP_DIAL: undefined,
  OP_SHUTDOWN: undefined,
  OP_LISTEN: undefined,
  OP_LISTEN_TLS: undefined,
  OP_RESOURCES: undefined,
  OP_GET_RANDOM_VALUES: undefined,
  OP_GLOBAL_TIMER_STOP: undefined,
  OP_GLOBAL_TIMER: undefined,
  OP_NOW: undefined,
  OP_QUERY_PERMISSION: undefined,
  OP_REVOKE_PERMISSION: undefined,
  OP_REQUEST_PERMISSION: undefined,
  OP_CREATE_WORKER: undefined,
  OP_HOST_GET_WORKER_LOADED: undefined,
  OP_HOST_POST_MESSAGE: undefined,
  OP_HOST_POLL_WORKER: undefined,
  OP_HOST_CLOSE_WORKER: undefined,
  OP_HOST_RESUME_WORKER: undefined,
  OP_HOST_GET_MESSAGE: undefined,
  OP_WORKER_POST_MESSAGE: undefined,
  OP_WORKER_GET_MESSAGE: undefined,
  OP_RUN: undefined,
  OP_RUN_STATUS: undefined,
  OP_KILL: undefined,
  OP_CHDIR: undefined,
  OP_MKDIR: undefined,
  OP_CHMOD: undefined,
  OP_CHOWN: undefined,
  OP_REMOVE: undefined,
  OP_COPY_FILE: undefined,
  OP_STAT: undefined,
  OP_REALPATH: undefined,
  OP_READ_DIR: undefined,
  OP_RENAME: undefined,
  OP_LINK: undefined,
  OP_SYMLINK: undefined,
  OP_READ_LINK: undefined,
  OP_TRUNCATE: undefined,
  OP_MAKE_TEMP_DIR: undefined,
  OP_CWD: undefined,
  OP_DIAL_TLS: undefined,
  OP_HOSTNAME: undefined,
  OP_OPEN_PLUGIN: undefined,
  OP_COMPILE: undefined,
  OP_TRANSPILE: undefined,
  OP_FETCH_ASSET: undefined,
};

export function setOps(opMap: { [key: string]: number }) {
  for (const [name, opId] of Object.entries(opMap)) {
    const opName = `OP_${name.toUpperCase()}`;
    OPS[opName] = opId;
  }
}

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
export let OP_RESOLVE_MODULES: number;
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
export let OP_HOST_GET_WORKER_LOADED: number;
export let OP_HOST_POST_MESSAGE: number;
export let OP_HOST_POLL_WORKER: number;
export let OP_HOST_CLOSE_WORKER: number;
export let OP_HOST_RESUME_WORKER: number;
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
export let OP_DIAL_TLS: number;
export let OP_HOSTNAME: number;
export let OP_OPEN_PLUGIN: number;
export let OP_COMPILE: number;
export let OP_TRANSPILE: number;

/** **WARNING:** This is only available during the snapshotting process and is
 * unavailable at runtime. */
export let OP_FETCH_ASSET: number;

const PLUGIN_ASYNC_HANDLER_MAP: Map<number, AsyncHandler> = new Map();

export function setPluginAsyncHandler(
  opId: number,
  handler: AsyncHandler
): void {
  PLUGIN_ASYNC_HANDLER_MAP.set(opId, handler);
}

export function getAsyncHandler(opName: string): (msg: Uint8Array) => void {
  switch (opName) {
    case "OP_WRITE":
    case "OP_READ":
      return minimal.asyncMsgFromRust;
    default:
      return json.asyncMsgFromRust;
  }
}
