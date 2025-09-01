// Copyright 2018-2025 the Deno authors. MIT license.

import type { Buffer } from "node:buffer";
import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";

const { PromisePrototypeThen } = primordials;

export function link(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
  callback: CallbackWithError,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  PromisePrototypeThen(
    Deno.link(existingPath, newPath),
    () => callback(null),
    callback,
  );
}

export const linkPromise = promisify(link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

export function linkSync(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  Deno.linkSync(existingPath, newPath);
}
