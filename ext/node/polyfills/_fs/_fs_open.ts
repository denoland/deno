// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { parseFileMode } from "ext:deno_node/internal/validators.mjs";
import {
  getValidatedPathToString,
  stringToFlags,
} from "ext:deno_node/internal/fs/utils.mjs";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import type { Buffer } from "node:buffer";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { op_node_open, op_node_open_sync } from "ext:core/ops";

const { Promise, PromisePrototypeThen } = primordials;

export type OpenFlags =
  | "a"
  | "ax"
  | "a+"
  | "ax+"
  | "as"
  | "as+"
  | "r"
  | "r+"
  | "rs"
  | "rs+"
  | "w"
  | "wx"
  | "w+"
  | "wx+"
  | number
  | string;

type OpenCallback = (err: Error | null, fd?: number) => void;

export function open(path: string | Buffer | URL, callback: OpenCallback): void;
export function open(
  path: string | Buffer | URL,
  flags: OpenFlags,
  callback: OpenCallback,
): void;
export function open(
  path: string | Buffer | URL,
  flags: OpenFlags,
  mode: number,
  callback: OpenCallback,
): void;
export function open(
  path: string | Buffer | URL,
  flags: OpenCallback | OpenFlags,
  mode?: OpenCallback | number,
  callback?: OpenCallback,
) {
  path = getValidatedPathToString(path);
  if (arguments.length < 3) {
    // deno-lint-ignore no-explicit-any
    callback = flags as any;
    flags = "r";
    mode = 0o666;
  } else if (typeof mode === "function") {
    callback = mode;
    mode = 0o666;
  } else {
    mode = parseFileMode(mode, "mode", 0o666);
  }
  flags = stringToFlags(flags);
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_node_open(path, flags, mode),
    (rid: number) => callback(null, rid),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "open", path })),
  );
}

export function openPromise(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  mode = 0o666,
): Promise<FileHandle> {
  return new Promise((resolve, reject) => {
    open(path, flags, mode, (err, fd) => {
      if (err) reject(err);
      else resolve(new FileHandle(fd as number));
    });
  });
}

export function openSync(path: string | Buffer | URL): number;
export function openSync(
  path: string | Buffer | URL,
  flags?: OpenFlags,
): number;
export function openSync(path: string | Buffer | URL, mode?: number): number;
export function openSync(
  path: string | Buffer | URL,
  flags?: OpenFlags,
  mode?: number,
): number;
export function openSync(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  maybeMode?: number,
) {
  path = getValidatedPathToString(path);
  flags = stringToFlags(flags);
  const mode = parseFileMode(maybeMode, "mode", 0o666);

  try {
    return op_node_open_sync(path, flags, mode);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "open", path });
  }
}
