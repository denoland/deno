// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  emitRecursiveRmdirWarning,
  getValidatedPathToString,
  validateRmdirOptions,
  validateRmOptions,
  validateRmOptionsSync,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  denoErrorToNodeError,
  ERR_FS_RMDIR_ENOTDIR,
} from "ext:deno_node/internal/errors.ts";
import { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { op_node_rmdir, op_node_rmdir_sync } from "ext:core/ops";

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmdirCallback = (err?: Error) => void;

export function rmdir(
  path: string | Buffer | URL,
  callback: rmdirCallback,
): void;
export function rmdir(
  path: string | Buffer | URL,
  options: rmdirOptions,
  callback: rmdirCallback,
): void;
export function rmdir(
  path: string | Buffer | URL,
  optionsOrCallback: rmdirOptions | rmdirCallback,
  maybeCallback?: rmdirCallback,
) {
  path = getValidatedPathToString(path);

  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : undefined;

  if (!callback) throw new Error("No callback function supplied");

  if (options?.recursive) {
    emitRecursiveRmdirWarning();
    validateRmOptions(
      path,
      { ...options, force: false },
      true,
      (err, options) => {
        if (err === false) {
          return callback(new ERR_FS_RMDIR_ENOTDIR(path));
        }
        if (err) {
          return callback(err);
        }

        Deno.remove(path, { recursive: options?.recursive })
          .then((_) => callback(), callback);
      },
    );
  } else {
    validateRmdirOptions(options);
    op_node_rmdir(path, { recursive: options?.recursive })
      .then((_) => callback(), (err: unknown) => {
        callback(
          denoErrorToNodeError(err as Error, { syscall: "rmdir", path }),
        );
      });
  }
}

export const rmdirPromise = promisify(rmdir) as (
  path: string | Buffer | URL,
  options?: rmdirOptions,
) => Promise<void>;

export function rmdirSync(path: string | Buffer | URL, options?: rmdirOptions) {
  path = getValidatedPathToString(path);
  if (options?.recursive) {
    emitRecursiveRmdirWarning();
    const optionsOrFalse = validateRmOptionsSync(path, {
      ...options,
      force: false,
    }, true);
    if (optionsOrFalse === false) {
      throw new ERR_FS_RMDIR_ENOTDIR(path);
    }
    Deno.removeSync(path, {
      recursive: true,
    });
  } else {
    validateRmdirOptions(options);
  }

  try {
    op_node_rmdir_sync(path);
  } catch (err) {
    throw (denoErrorToNodeError(err as Error, { syscall: "rmdir", path }));
  }
}
