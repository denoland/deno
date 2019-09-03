// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { DispatchMinimalOp } from "./dispatch_minimal.ts";
import * as json from "./dispatch_json.ts";
import { core } from "./core.ts";
import { ops } from "./ops.ts";

const opNamespace = "builtins";

// These consts are shared with Rust. Update with care.
export const OP_READ = new DispatchMinimalOp(opNamespace, "read");
export const OP_WRITE = new DispatchMinimalOp(opNamespace, "write");
export let JSON_OP: number | undefined;
ops[opNamespace].jsonOp = (id?: number): void => {
  JSON_OP = id;
  if (id !== undefined) {
    core.setAsyncHandler(id, json.asyncMsgFromRust);
  }
};

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
export const OP_FETCH_ASSET = 57;
