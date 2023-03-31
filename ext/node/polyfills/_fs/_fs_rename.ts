// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { fromFileUrl } from "ext:deno_node/path.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function rename(
  oldPath: string | URL,
  newPath: string | URL,
  callback: (err?: Error) => void,
) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  if (!callback) throw new Error("No callback function supplied");

  denoFs.rename(oldPath, newPath).then((_) => callback(), callback);
}

export const renamePromise = promisify(rename) as (
  oldPath: string | URL,
  newPath: string | URL,
) => Promise<void>;

export function renameSync(oldPath: string | URL, newPath: string | URL) {
  oldPath = oldPath instanceof URL ? fromFileUrl(oldPath) : oldPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  denoFs.renameSync(oldPath, newPath);
}
