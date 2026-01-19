// Copyright 2018-2026 the Deno authors. MIT license.

import {
  validateRmOptions,
  validateRmOptionsSync,
} from "ext:deno_node/internal/fs/utils.mjs";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";

const {
  Error,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
} = primordials;

type rmOptions = {
  force?: boolean;
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmCallback = (err: Error | null) => void;

export function rm(path: string | URL, callback: rmCallback): void;
export function rm(
  path: string | URL,
  options: rmOptions,
  callback: rmCallback,
): void;
export function rm(
  path: string | URL,
  optionsOrCallback: rmOptions | rmCallback,
  maybeCallback?: rmCallback,
) {
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : undefined;

  if (!callback) throw new Error("No callback function supplied");

  validateRmOptions(
    path,
    options,
    false,
    (err: Error | null, options: rmOptions) => {
      if (err) {
        return callback(err);
      }

      PromisePrototypeThen(
        Deno.remove(path, { recursive: options?.recursive }),
        () => callback(null),
        (err) => {
          if (
            options?.force &&
            ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
          ) {
            return callback(null);
          }

          callback(denoErrorToNodeError(err, { syscall: "rm" }));
        },
      );
    },
  );
}

export const rmPromise = promisify(rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

export function rmSync(path: string | URL, options?: rmOptions) {
  options = validateRmOptionsSync(path, options, false);
  try {
    Deno.removeSync(path, { recursive: options?.recursive });
  } catch (err: unknown) {
    if (
      options?.force &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    throw denoErrorToNodeError(err, { syscall: "rm" });
  }
}
