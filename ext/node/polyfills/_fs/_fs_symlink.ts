// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { promisify } from "ext:deno_node/internal/util.mjs";

const {
  Error,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
} = primordials;

type SymlinkType = "file" | "dir" | "junction";

export function symlink(
  target: string | URL,
  path: string | URL,
  typeOrCallback: SymlinkType | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  target = ObjectPrototypeIsPrototypeOf(URL, target)
    ? pathFromURL(target)
    : target;
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;

  const type: SymlinkType = typeof typeOrCallback === "string"
    ? typeOrCallback
    : "file";

  const callback: CallbackWithError = typeof typeOrCallback === "function"
    ? typeOrCallback
    : (maybeCallback as CallbackWithError);

  if (!callback) throw new Error("No callback function supplied");

  PromisePrototypeThen(
    Deno.symlink(target, path, { type }),
    () => callback(null),
    callback,
  );
}

export const symlinkPromise = promisify(symlink) as (
  target: string | URL,
  path: string | URL,
  type?: SymlinkType,
) => Promise<void>;

export function symlinkSync(
  target: string | URL,
  path: string | URL,
  type?: SymlinkType,
) {
  target = ObjectPrototypeIsPrototypeOf(URL, target)
    ? pathFromURL(target)
    : target;
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;
  type = type || "file";

  Deno.symlinkSync(target, path, { type });
}
