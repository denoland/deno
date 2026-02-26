// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import {
  parseFileMode,
  validateBoolean,
} from "ext:deno_node/internal/validators.mjs";
import { resolve } from "node:path";

type MkdirCallback =
  | ((err: Error | null, path?: string) => void)
  | CallbackWithError;

/** Find the first component of `path` that does not exist. */
function findFirstNonExistent(path: string): string | undefined {
  let cursor = resolve(path);
  while (true) {
    try {
      Deno.statSync(cursor);
      // cursor exists, so the first non-existent is one level deeper
      return undefined;
    } catch {
      const parent = resolve(cursor, "..");
      if (parent === cursor) {
        // reached filesystem root - nothing exists
        return cursor;
      }
      // Check if the parent exists
      try {
        Deno.statSync(parent);
        // parent exists but cursor doesn't - cursor is first non-existent
        return cursor;
      } catch {
        // parent also doesn't exist, keep going up
        cursor = parent;
      }
    }
  }
}

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

export function mkdir(
  path: string | URL,
  options?: MkdirOptions | MkdirCallback,
  callback?: MkdirCallback,
) {
  path = getValidatedPath(path) as string;

  let mode = 0o777;
  let recursive = false;

  if (typeof options == "function") {
    callback = options;
  } else if (typeof options === "number") {
    mode = parseFileMode(options, "mode");
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) {
      mode = parseFileMode(options.mode, "options.mode");
    }
  }
  validateBoolean(recursive, "options.recursive");

  let firstNonExistent: string | undefined;
  try {
    firstNonExistent = recursive ? findFirstNonExistent(path) : undefined;
  } catch (err) {
    if (typeof callback === "function") {
      callback(
        denoErrorToNodeError(err as Error, { syscall: "mkdir", path }),
      );
    }
    return;
  }

  Deno.mkdir(path, { recursive, mode })
    .then(() => {
      if (typeof callback === "function") {
        callback(null, firstNonExistent);
      }
    }, (err) => {
      if (typeof callback === "function") {
        callback(err);
      }
    });
}

export const mkdirPromise = promisify(mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

export function mkdirSync(
  path: string | URL,
  options?: MkdirOptions,
): string | undefined {
  path = getValidatedPath(path) as string;

  let mode = 0o777;
  let recursive = false;

  if (typeof options === "number") {
    mode = parseFileMode(options, "mode");
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) {
      mode = parseFileMode(options.mode, "options.mode");
    }
  }
  validateBoolean(recursive, "options.recursive");

  let firstNonExistent: string | undefined;
  try {
    firstNonExistent = recursive ? findFirstNonExistent(path) : undefined;
    Deno.mkdirSync(path, { recursive, mode });
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "mkdir", path });
  }

  return firstNonExistent;
}
