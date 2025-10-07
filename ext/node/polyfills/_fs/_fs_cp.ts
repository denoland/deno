// Copyright 2018-2025 the Deno authors. MIT license.
import { primordials } from "ext:core/mod.js";
import {
  getValidatedPathToString,
  validateCpOptions,
} from "ext:deno_node/internal/fs/utils.mjs";
import { cpFn } from "ext:deno_node/_fs/cp/cp.ts";
import { cpSyncFn } from "ext:deno_node/_fs/cp/cp_sync.ts";
import type {
  CopyOptions,
  CopySyncOptions,
} from "ext:deno_node/_fs/cp/cp.d.ts";
import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";

const { PromisePrototypeThen } = primordials;

export function cpSync(
  src: string | URL,
  dest: string | URL,
  options: CopySyncOptions,
) {
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");

  cpSyncFn(srcPath, destPath, options);
}

export function cp(
  src: string | URL,
  dest: string | URL,
  options: CopyOptions | undefined,
  callback: CallbackWithError,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  callback = makeCallback(callback);
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");

  PromisePrototypeThen(
    cpFn(srcPath, destPath, options),
    () => callback(null),
    callback,
  );
}

export async function cpPromise(
  src: string | URL,
  dest: string | URL,
  options?: CopyOptions,
): Promise<void> {
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");
  return await cpFn(srcPath, destPath, options);
}
