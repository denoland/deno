// Copyright 2018-2025 the Deno authors. MIT license.

import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import {
  parseFileMode,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";
import { op_fs_fchmod_async, op_fs_fchmod_sync } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import { promisify } from "ext:deno_node/internal/util.mjs";

const { PromisePrototypeThen } = primordials;

export function fchmod(
  fd: number,
  mode: string | number,
  callback: CallbackWithError,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  mode = parseFileMode(mode, "mode");
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_fs_fchmod_async(fd, mode),
    () => callback(null),
    callback,
  );
}

export function fchmodSync(fd: number, mode: string | number) {
  validateInteger(fd, "fd", 0, 2147483647);

  op_fs_fchmod_sync(fd, parseFileMode(mode, "mode"));
}

export const fchmodPromise = promisify(fchmod) as (
  fd: number,
  mode: string | number,
) => Promise<void>;
