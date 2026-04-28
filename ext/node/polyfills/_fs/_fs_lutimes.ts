// Copyright 2018-2026 the Deno authors. MIT license.

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { type Buffer } from "node:buffer";
import { primordials } from "ext:core/mod.js";
import { op_node_lutimes, op_node_lutimes_sync } from "ext:core/ops";
import { promisify } from "ext:deno_node/internal/util.mjs";
import {
  getValidatedPathToString,
  toUnixTimestamp,
} from "ext:deno_node/internal/fs/utils.mjs";

const {
  Error,
  MathTrunc,
  Number,
  NumberIsFinite,
  NumberIsNaN,
  PromisePrototypeThen,
} = primordials;

type TimeLike = number | string | Date;
type PathLike = string | Buffer | URL;

function getValidUnixTime(
  value: TimeLike,
  name: string,
): [number, number] {
  if (typeof value === "string") {
    value = Number(value);
  }

  if (
    typeof value === "number" &&
    (NumberIsNaN(value) || !NumberIsFinite(value))
  ) {
    throw new Deno.errors.InvalidData(
      `invalid ${name}, must not be infinity or NaN`,
    );
  }

  const unixSeconds = toUnixTimestamp(value);

  const seconds = MathTrunc(unixSeconds);
  const nanoseconds = MathTrunc((unixSeconds * 1e3) - (seconds * 1e3)) * 1e6;

  return [
    seconds,
    nanoseconds,
  ];
}

export function lutimes(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
  callback: CallbackWithError,
): void {
  if (!callback) {
    throw new Error("No callback function supplied");
  }
  const { 0: atimeSecs, 1: atimeNanos } = getValidUnixTime(atime, "atime");
  const { 0: mtimeSecs, 1: mtimeNanos } = getValidUnixTime(mtime, "mtime");

  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    op_node_lutimes(path, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos),
    () => callback(null),
    callback,
  );
}

export function lutimesSync(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
): void {
  const { 0: atimeSecs, 1: atimeNanos } = getValidUnixTime(atime, "atime");
  const { 0: mtimeSecs, 1: mtimeNanos } = getValidUnixTime(mtime, "mtime");

  path = getValidatedPathToString(path);

  op_node_lutimes_sync(path, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos);
}

export const lutimesPromise = promisify(lutimes) as (
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
) => Promise<void>;
