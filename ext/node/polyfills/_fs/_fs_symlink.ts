// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { fromFileUrl } from "ext:deno_node/path.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";

type SymlinkType = "file" | "dir";

export function symlink(
  target: string | URL,
  path: string | URL,
  typeOrCallback: SymlinkType | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  target = target instanceof URL ? fromFileUrl(target) : target;
  path = path instanceof URL ? fromFileUrl(path) : path;

  const type: SymlinkType = typeof typeOrCallback === "string"
    ? typeOrCallback
    : "file";

  const callback: CallbackWithError = typeof typeOrCallback === "function"
    ? typeOrCallback
    : (maybeCallback as CallbackWithError);

  if (!callback) throw new Error("No callback function supplied");

  Deno.symlink(target, path, { type }).then(() => callback(null), callback);
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
  target = target instanceof URL ? fromFileUrl(target) : target;
  path = path instanceof URL ? fromFileUrl(path) : path;
  type = type || "file";

  Deno.symlinkSync(target, path, { type });
}
