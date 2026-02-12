// Copyright 2018-2026 the Deno authors. MIT license.

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
import { primordials } from "ext:core/mod.js";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";

const { PromisePrototypeThen } = primordials;

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
  options: rmdirOptions | rmdirCallback | undefined,
  callback?: rmdirCallback,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  validateFunction(callback, "cb");
  path = getValidatedPathToString(path);

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

        PromisePrototypeThen(
          Deno.remove(path, { recursive: options?.recursive }),
          (_) => callback(),
          (err: Error) =>
            callback(
              denoErrorToNodeError(err as Error, { syscall: "rmdir", path }),
            ),
        );
      },
    );
  } else {
    validateRmdirOptions(options);
    PromisePrototypeThen(
      op_node_rmdir(path),
      (_) => callback(),
      (err: Error) =>
        callback(
          denoErrorToNodeError(err as Error, { syscall: "rmdir", path }),
        ),
    );
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
