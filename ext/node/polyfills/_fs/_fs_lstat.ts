// Copyright 2018-2026 the Deno authors. MIT license.

import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import {
  CFISBIS,
  type statCallback,
  type statCallbackBigInt,
  type statOptions,
} from "ext:deno_node/internal/fs/stat_utils.ts";
import {
  BigIntStats,
  getValidatedPathToString,
  Stats,
} from "ext:deno_node/internal/fs/utils.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";

const {
  Error,
  PromisePrototypeThen,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

export function lstat(path: string | URL, callback: statCallback): void;
export function lstat(
  path: string | URL,
  options: { bigint: false },
  callback: statCallback,
): void;
export function lstat(
  path: string | URL,
  options: { bigint: true },
  callback: statCallbackBigInt,
): void;
export function lstat(
  path: string | URL,
  optionsOrCallback: statCallback | statCallbackBigInt | statOptions,
  maybeCallback?: statCallback | statCallbackBigInt,
) {
  const callback =
    (typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback) as (
        ...args: [Error] | [null, BigIntStats | Stats]
      ) => void;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : { bigint: false };

  if (!callback) throw new Error("No callback function supplied");

  // Match Node: errors carry the requested path (see lib/fs.js lstat).
  const validatedPath = getValidatedPathToString(path);
  PromisePrototypeThen(
    Deno.lstat(validatedPath),
    (stat) => callback(null, CFISBIS(stat, options.bigint)),
    (err) =>
      callback(
        denoErrorToNodeError(err, { syscall: "lstat", path: validatedPath }),
      ),
  );
}

export const lstatPromise = promisify(lstat) as (
  & ((path: string | URL) => Promise<Stats>)
  & ((path: string | URL, options: { bigint: false }) => Promise<Stats>)
  & ((path: string | URL, options: { bigint: true }) => Promise<BigIntStats>)
);

export function lstatSync(path: string | URL): Stats;
export function lstatSync(
  path: string | URL,
  options: { bigint: false; throwIfNoEntry?: boolean },
): Stats;
export function lstatSync(
  path: string | URL,
  options: { bigint: true; throwIfNoEntry?: boolean },
): BigIntStats;
export function lstatSync(
  path: string | URL,
  options?: statOptions,
): Stats | BigIntStats {
  const validatedPath = getValidatedPathToString(path);
  try {
    const origin = Deno.lstatSync(validatedPath);
    return CFISBIS(origin, options?.bigint || false);
  } catch (err) {
    if (
      options?.throwIfNoEntry === false &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    throw denoErrorToNodeError(err, { syscall: "lstat", path: validatedPath });
  }
}
