// Copyright 2018-2026 the Deno authors. MIT license.

import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import { getValidatedFd } from "ext:deno_node/internal/fs/utils.mjs";
import { getRid, unregisterFd } from "ext:deno_node/internal/fs/fd_map.ts";
import { core, primordials } from "ext:core/mod.js";

const {
  Error,
  ErrorPrototype,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

function defaultCloseCallback(err: Error | null) {
  if (err !== null) throw err;
}

export function close(
  fd: number,
  callback: CallbackWithError = defaultCloseCallback,
) {
  fd = getValidatedFd(fd);
  if (callback !== defaultCloseCallback) {
    callback = makeCallback(callback);
  }

  setTimeout(() => {
    let error = null;
    try {
      const rid = getRid(fd);
      core.close(rid);
      unregisterFd(fd);
    } catch (err) {
      error = ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)
        ? err as Error
        : new Error("[non-error thrown]");
    }
    callback(error);
  }, 0);
}

export function closeSync(fd: number) {
  fd = getValidatedFd(fd);
  const rid = getRid(fd);
  core.close(rid);
  unregisterFd(fd);
}
