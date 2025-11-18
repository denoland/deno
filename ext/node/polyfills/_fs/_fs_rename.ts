// Copyright 2018-2025 the Deno authors. MIT license.

import { promisify } from "ext:deno_node/internal/util.mjs";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { primordials } from "ext:core/mod.js";
import type { Buffer } from "node:buffer";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";

const {
  PromisePrototypeThen,
} = primordials;

export function rename(
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
  callback: (err?: Error) => void,
) {
  oldPath = getValidatedPathToString(oldPath, "oldPath");
  newPath = getValidatedPathToString(newPath, "newPath");
  validateFunction(callback, "callback");

  PromisePrototypeThen(
    Deno.rename(oldPath, newPath),
    () => callback(),
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "rename",
        path: oldPath,
        dest: newPath,
      })),
  );
}

export const renamePromise = promisify(rename) as (
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

export function renameSync(
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  oldPath = getValidatedPathToString(oldPath, "oldPath");
  newPath = getValidatedPathToString(newPath, "newPath");

  try {
    Deno.renameSync(oldPath, newPath);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "rename",
      path: oldPath,
      dest: newPath,
    });
  }
}
