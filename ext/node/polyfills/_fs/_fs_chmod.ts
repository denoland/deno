// Copyright 2018-2025 the Deno authors. MIT license.

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { parseFileMode } from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { primordials } from "ext:core/mod.js";

const { PromisePrototypeThen } = primordials;

export function chmod(
  path: string | Buffer | URL,
  mode: string | number,
  callback: CallbackWithError,
) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  PromisePrototypeThen(
    Deno.chmod(path, mode),
    () => callback(null),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "chmod", path })),
  );
}

export const chmodPromise = promisify(chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

export function chmodSync(path: string | Buffer | URL, mode: string | number) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  try {
    Deno.chmodSync(path, mode);
  } catch (error) {
    throw denoErrorToNodeError(error as Error, { syscall: "chmod", path });
  }
}
