// Copyright 2018-2025 the Deno authors. MIT license.

import { type Buffer } from "node:buffer";
import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import { ERR_METHOD_NOT_IMPLEMENTED } from "ext:deno_node/internal/errors.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { isMacOS } from "ext:deno_node/_util/os.ts";
import { op_node_lchmod, op_node_lchmod_sync } from "ext:core/ops";
import { parseFileMode } from "ext:deno_node/internal/validators.mjs";
import { primordials } from "ext:core/mod.js";
import { promisify } from "ext:deno_node/internal/util.mjs";

const { PromisePrototypeThen, PromiseReject } = primordials;

export const lchmod = !isMacOS ? undefined : (
  path: string | Buffer | URL,
  mode: number,
  callback: CallbackWithError,
) => {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_node_lchmod(path, mode),
    () => callback(null),
    (err) => callback(err),
  );
};

export const lchmodPromise = !isMacOS
  ? () => PromiseReject(new ERR_METHOD_NOT_IMPLEMENTED("lchmod()"))
  : promisify(lchmod) as (
    path: string | Buffer | URL,
    mode: number,
  ) => Promise<void>;

export const lchmodSync = !isMacOS
  ? undefined
  : (path: string | Buffer | URL, mode: number) => {
    path = getValidatedPathToString(path);
    mode = parseFileMode(mode, "mode");
    return op_node_lchmod_sync(path, mode);
  };
