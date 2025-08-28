// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

import {
  CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import type { Buffer } from "node:buffer";
import { validateOneOf } from "ext:deno_node/internal/validators.mjs";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { op_node_symlink, op_node_symlink_sync } from "ext:core/ops";

const { PromisePrototypeThen } = primordials;

export type SymlinkType = "file" | "dir" | "junction";

export function symlink(
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  linkType?: SymlinkType | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (callback === undefined) {
    callback = linkType as CallbackWithError;
    linkType = undefined;
  } else {
    validateOneOf(linkType, "type", [
      "dir",
      "file",
      "junction",
      null,
      undefined,
    ]);
  }

  callback = makeCallback(callback);
  target = getValidatedPathToString(target, "target");
  path = getValidatedPathToString(path);
  linkType ??= undefined;

  PromisePrototypeThen(
    op_node_symlink(target, path, linkType),
    () => callback(null),
    callback,
  );
}

export const symlinkPromise = promisify(symlink) as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) => Promise<void>;

export function symlinkSync(
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) {
  validateOneOf(type, "type", ["dir", "file", "junction", null, undefined]);
  target = getValidatedPathToString(target, "target");
  path = getValidatedPathToString(path);
  type ??= undefined;

  op_node_symlink_sync(target, path, type);
}
