// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { promisify } from "ext:deno_node/internal/util.mjs";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function unlink(path: string | URL, callback: (err?: Error) => void) {
  if (!callback) throw new Error("No callback function supplied");
  denoFs.remove(path).then((_) => callback(), callback);
}

export const unlinkPromise = promisify(unlink) as (
  path: string | URL,
) => Promise<void>;

export function unlinkSync(path: string | URL) {
  denoFs.removeSync(path);
}
