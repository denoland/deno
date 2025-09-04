// Copyright 2018-2025 the Deno authors. MIT license.

import { promisify } from "ext:deno_node/internal/util.mjs";
import type { Buffer } from "node:buffer";
import { primordials } from "ext:core/mod.js";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";

const {
  PromisePrototypeThen,
} = primordials;

export function unlink(
  path: string | Buffer | URL,
  callback: (err?: Error) => void,
): void {
  path = getValidatedPathToString(path);
  validateFunction(callback, "callback");

  PromisePrototypeThen(
    Deno.remove(path),
    () => callback(),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "unlink", path })),
  );
}

export const unlinkPromise = promisify(unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

export function unlinkSync(path: string | Buffer | URL): void {
  path = getValidatedPathToString(path);
  try {
    Deno.removeSync(path);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "unlink", path });
  }
}
