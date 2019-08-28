// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./dispatch_minimal";
import * as json from "./dispatch_json";
import { core } from "./core";
import { ops } from "./ops";

const opNamespace = ops.builtins;

type MaybeOpId = number | undefined;

type OpId = number;

// These consts are shared with Rust. Update with care.
export let OP_READ: OpId;
opNamespace.read = (id: MaybeOpId): void => {
  OP_READ = id!;
  core.setAsyncHandler(
    id!,
    (buf: Uint8Array): void => minimal.asyncMsgFromRust(id!, buf)
  );
};
export let OP_WRITE: OpId;
opNamespace.write = (id: MaybeOpId): void => {
  OP_WRITE = id!;
  core.setAsyncHandler(
    id!,
    (buf: Uint8Array): void => minimal.asyncMsgFromRust(id!, buf)
  );
};
export let OP_EXIT: OpId;
opNamespace.exit = (id: MaybeOpId): void => {
  OP_EXIT = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_IS_TTY: OpId;
opNamespace.isTty = (id: MaybeOpId): void => {
  OP_IS_TTY = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_ENV: OpId;
opNamespace.env = (id: MaybeOpId): void => {
  OP_ENV = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_EXEC_PATH: OpId;
opNamespace.execPath = (id: MaybeOpId): void => {
  OP_EXEC_PATH = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_UTIME: OpId;
opNamespace.utime = (id: MaybeOpId): void => {
  OP_UTIME = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_SET_ENV: OpId;
opNamespace.setEnv = (id: MaybeOpId): void => {
  OP_SET_ENV = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_HOME_DIR: OpId;
opNamespace.homeDir = (id: MaybeOpId): void => {
  OP_HOME_DIR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_START: OpId;
opNamespace.start = (id: MaybeOpId): void => {
  OP_START = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_APPLY_SOURCE_MAP: OpId;
opNamespace.applySourceMap = (id: MaybeOpId): void => {
  OP_APPLY_SOURCE_MAP = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_FORMAT_ERROR: OpId;
opNamespace.formatError = (id: MaybeOpId): void => {
  OP_FORMAT_ERROR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CACHE: OpId;
opNamespace.cache = (id: MaybeOpId): void => {
  OP_CACHE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_FETCH_SOURCE_FILE: OpId;
opNamespace.fetchSourceFile = (id: MaybeOpId): void => {
  OP_FETCH_SOURCE_FILE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_OPEN: OpId;
opNamespace.open = (id: MaybeOpId): void => {
  OP_OPEN = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CLOSE: OpId;
opNamespace.close = (id: MaybeOpId): void => {
  OP_CLOSE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_SEEK: OpId;
opNamespace.seek = (id: MaybeOpId): void => {
  OP_SEEK = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_FETCH: OpId;
opNamespace.fetch = (id: MaybeOpId): void => {
  OP_FETCH = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_METRICS: OpId;
opNamespace.metrics = (id: MaybeOpId): void => {
  OP_METRICS = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_REPL_START: OpId;
opNamespace.replStart = (id: MaybeOpId): void => {
  OP_REPL_START = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_REPL_READLINE: OpId;
opNamespace.replReadline = (id: MaybeOpId): void => {
  OP_REPL_READLINE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_ACCEPT: OpId;
opNamespace.accept = (id: MaybeOpId): void => {
  OP_ACCEPT = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_DIAL: OpId;
opNamespace.dial = (id: MaybeOpId): void => {
  OP_DIAL = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_SHUTDOWN: OpId;
opNamespace.shutdown = (id: MaybeOpId): void => {
  OP_SHUTDOWN = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_LISTEN: OpId;
opNamespace.listen = (id: MaybeOpId): void => {
  OP_LISTEN = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_RESOURCES: OpId;
opNamespace.resources = (id: MaybeOpId): void => {
  OP_RESOURCES = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_GET_RANDOM_VALUES: OpId;
opNamespace.getRandomValues = (id: MaybeOpId): void => {
  OP_GET_RANDOM_VALUES = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_GLOBAL_TIMER_STOP: OpId;
opNamespace.globalTimerStop = (id: MaybeOpId): void => {
  OP_GLOBAL_TIMER_STOP = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_GLOBAL_TIMER: OpId;
opNamespace.globalTimer = (id: MaybeOpId): void => {
  OP_GLOBAL_TIMER = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_NOW: OpId;
opNamespace.now = (id: MaybeOpId): void => {
  OP_NOW = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_PERMISSIONS: OpId;
opNamespace.permissions = (id: MaybeOpId): void => {
  OP_PERMISSIONS = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_REVOKE_PERMISSION: OpId;
opNamespace.revokePermission = (id: MaybeOpId): void => {
  OP_REVOKE_PERMISSION = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CREATE_WORKER: OpId;
opNamespace.createWorker = (id: MaybeOpId): void => {
  OP_CREATE_WORKER = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_HOST_GET_WORKER_CLOSED: OpId;
opNamespace.hostGetWorkerClosed = (id: MaybeOpId): void => {
  OP_HOST_GET_WORKER_CLOSED = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_HOST_POST_MESSAGE: OpId;
opNamespace.hostPostMessage = (id: MaybeOpId): void => {
  OP_HOST_POST_MESSAGE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_HOST_GET_MESSAGE: OpId;
opNamespace.hostGetMessage = (id: MaybeOpId): void => {
  OP_HOST_GET_MESSAGE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_WORKER_POST_MESSAGE: OpId;
opNamespace.workerPostMessage = (id: MaybeOpId): void => {
  OP_WORKER_POST_MESSAGE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_WORKER_GET_MESSAGE: OpId;
opNamespace.workerGetMessage = (id: MaybeOpId): void => {
  OP_WORKER_GET_MESSAGE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_RUN: OpId;
opNamespace.run = (id: MaybeOpId): void => {
  OP_RUN = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_RUN_STATUS: OpId;
opNamespace.runStatus = (id: MaybeOpId): void => {
  OP_RUN_STATUS = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_KILL: OpId;
opNamespace.kill = (id: MaybeOpId): void => {
  OP_KILL = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CHDIR: OpId;
opNamespace.chdir = (id: MaybeOpId): void => {
  OP_CHDIR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_MKDIR: OpId;
opNamespace.mkdir = (id: MaybeOpId): void => {
  OP_MKDIR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CHMOD: OpId;
opNamespace.chmod = (id: MaybeOpId): void => {
  OP_CHMOD = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CHOWN: OpId;
opNamespace.chown = (id: MaybeOpId): void => {
  OP_CHOWN = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_REMOVE: OpId;
opNamespace.remove = (id: MaybeOpId): void => {
  OP_REMOVE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_COPY_FILE: OpId;
opNamespace.copyFile = (id: MaybeOpId): void => {
  OP_COPY_FILE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_STAT: OpId;
opNamespace.stat = (id: MaybeOpId): void => {
  OP_STAT = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_READ_DIR: OpId;
opNamespace.readDir = (id: MaybeOpId): void => {
  OP_READ_DIR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_RENAME: OpId;
opNamespace.rename = (id: MaybeOpId): void => {
  OP_RENAME = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_LINK: OpId;
opNamespace.link = (id: MaybeOpId): void => {
  OP_LINK = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_SYMLINK: OpId;
opNamespace.symlink = (id: MaybeOpId): void => {
  OP_SYMLINK = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_READ_LINK: OpId;
opNamespace.readLink = (id: MaybeOpId): void => {
  OP_READ_LINK = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_TRUNCATE: OpId;
opNamespace.truncate = (id: MaybeOpId): void => {
  OP_TRUNCATE = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_MAKE_TEMP_DIR: OpId;
opNamespace.makeTempDir = (id: MaybeOpId): void => {
  OP_MAKE_TEMP_DIR = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_CWD: OpId;
opNamespace.cwd = (id: MaybeOpId): void => {
  OP_CWD = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
export let OP_FETCH_ASSET: OpId;
opNamespace.fetchAsset = (id: MaybeOpId): void => {
  OP_FETCH_ASSET = id!;
  core.setAsyncHandler(id!, json.asyncMsgFromRust);
};
