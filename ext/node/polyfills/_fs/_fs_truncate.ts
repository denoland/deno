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

export function truncate(
  path: string | URL,
  lenOrCallback: number | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;
  const len: number | undefined = typeof lenOrCallback === "number"
    ? lenOrCallback
    : undefined;
  const callback: CallbackWithError = typeof lenOrCallback === "function"
    ? lenOrCallback
    : maybeCallback as CallbackWithError;

  if (!callback) throw new Error("No callback function supplied");

  PromisePrototypeThen(
    Deno.truncate(path, len),
    () => callback(null),
    callback,
  );
}

export const truncatePromise = promisify(truncate) as (
  path: string | URL,
  len?: number,
) => Promise<void>;

export function truncateSync(path: string | URL, len?: number) {
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;

  Deno.truncateSync(path, len);
}
