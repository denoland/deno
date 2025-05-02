// Copyright 2018-2025 the Deno authors. MIT license.

import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";

type Options = { encoding: string };
type Callback = (err: Error | null, path?: string) => void;

const { PromisePrototypeThen, Error } = primordials;

export function realpath(
  path: string,
  options?: Options | Callback,
  callback?: Callback,
) {
  if (typeof options === "function") {
    callback = options;
  }
  if (!callback) {
    throw new Error("No callback function supplied");
  }
  PromisePrototypeThen(
    Deno.realPath(path),
    (path) => callback!(null, path),
    (err) => callback!(err),
  );
}

realpath.native = realpath;

export const realpathPromise = promisify(realpath) as (
  path: string,
  options?: Options,
) => Promise<string>;

export function realpathSync(path: string): string {
  return Deno.realPathSync(path);
}

realpathSync.native = realpathSync;
