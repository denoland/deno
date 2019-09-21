// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal.ts";
import * as json from "./dispatch_json.ts";

// These consts are shared with Rust. Update with care.
Deno.core.refreshOpsMap();
export const OP_READ = Deno.core.opsMap["read"];
export const OP_WRITE = Deno.core.opsMap["write"];
export const OP_EXIT = Deno.core.opsMap["exit"];
export const OP_IS_TTY = Deno.core.opsMap["is_tty"];
export const OP_ENV = Deno.core.opsMap["env"];
export const OP_EXEC_PATH = Deno.core.opsMap["exec_path"];
export const OP_UTIME = Deno.core.opsMap["utime"];
export const OP_SET_ENV = Deno.core.opsMap["set_env"];
export const OP_HOME_DIR = Deno.core.opsMap["home_dir"];
export const OP_START = Deno.core.opsMap["start"];
export const OP_APPLY_SOURCE_MAP = Deno.core.opsMap["apply_source_map"];
export const OP_FORMAT_ERROR = Deno.core.opsMap["op_format_error"];
export const OP_CACHE = Deno.core.opsMap["op_cache"];
export const OP_FETCH_SOURCE_FILES = Deno.core.opsMap["op_fetch_source_files"];
export const OP_OPEN = Deno.core.opsMap["open"];
export const OP_CLOSE = Deno.core.opsMap["close"];
export const OP_SEEK = Deno.core.opsMap["seek"];
export const OP_FETCH = Deno.core.opsMap["fetch"];
export const OP_METRICS = Deno.core.opsMap["metrics"];
export const OP_REPL_START = Deno.core.opsMap["repl_start"];
export const OP_REPL_READLINE = Deno.core.opsMap["repl_readline"];
export const OP_ACCEPT = Deno.core.opsMap["accept"];
export const OP_DIAL = Deno.core.opsMap["dial"];
export const OP_SHUTDOWN = Deno.core.opsMap["shutdown"];
export const OP_LISTEN = Deno.core.opsMap["listen"];
export const OP_RESOURCES = Deno.core.opsMap["resources"];
export const OP_GET_RANDOM_VALUES = Deno.core.opsMap["get_random_values"];
export const OP_GLOBAL_TIMER_STOP = Deno.core.opsMap["global_timer_stop"];
export const OP_GLOBAL_TIMER = Deno.core.opsMap["global_timer"];
export const OP_NOW = Deno.core.opsMap["now"];
export const OP_PERMISSIONS = Deno.core.opsMap["permissions"];
export const OP_REVOKE_PERMISSION = Deno.core.opsMap["revoke_permission"];
export const OP_CREATE_WORKER = Deno.core.opsMap["create_worker"];
export const OP_HOST_GET_WORKER_CLOSED =
  Deno.core.opsMap["host_get_worker_closed"];
export const OP_HOST_POST_MESSAGE = Deno.core.opsMap["host_post_message"];
export const OP_HOST_GET_MESSAGE = Deno.core.opsMap["host_get_message"];
export const OP_WORKER_POST_MESSAGE = Deno.core.opsMap["worker_post_message"];
export const OP_WORKER_GET_MESSAGE = Deno.core.opsMap["worker_get_message"];
export const OP_RUN = Deno.core.opsMap["run"];
export const OP_RUN_STATUS = Deno.core.opsMap["run_status"];
export const OP_KILL = Deno.core.opsMap["kill"];
export const OP_CHDIR = Deno.core.opsMap["chdir"];
export const OP_MKDIR = Deno.core.opsMap["mkdir"];
export const OP_CHMOD = Deno.core.opsMap["chmod"];
export const OP_CHOWN = Deno.core.opsMap["chown"];
export const OP_REMOVE = Deno.core.opsMap["remove"];
export const OP_COPY_FILE = Deno.core.opsMap["copy_file"];
export const OP_STAT = Deno.core.opsMap["stat"];
export const OP_READ_DIR = Deno.core.opsMap["read_dir"];
export const OP_RENAME = Deno.core.opsMap["rename"];
export const OP_LINK = Deno.core.opsMap["link"];
export const OP_SYMLINK = Deno.core.opsMap["symlink"];
export const OP_READ_LINK = Deno.core.opsMap["read_link"];
export const OP_TRUNCATE = Deno.core.opsMap["truncate"];
export const OP_MAKE_TEMP_DIR = Deno.core.opsMap["make_temp_dir"];
export const OP_CWD = Deno.core.opsMap["cwd"];
export const OP_FETCH_ASSET = Deno.core.opsMap["fetch_asset"];

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
