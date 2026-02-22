// Copyright 2018-2026 the Deno authors. MIT license.

import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import {
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import { op_fs_read_file_async } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";

const { PromisePrototypeThen } = primordials;

/**
 * Returns a `Blob` whose data is read from the given file.
 */
export function openAsBlob(
  path: string | Buffer | URL,
  options: { type?: string } = { __proto__: null },
): Promise<Blob> {
  validateObject(options, "options");
  const type = options.type || "";
  validateString(type, "options.type");
  path = getValidatedPath(path);
  return PromisePrototypeThen(
    op_fs_read_file_async(path as string, undefined, 0),
    (data: Uint8Array) => new Blob([data], { type }),
  );
}
