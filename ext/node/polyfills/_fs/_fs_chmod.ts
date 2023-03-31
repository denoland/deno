// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const core = globalThis.Deno.core;
import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import * as pathModule from "ext:deno_node/path.ts";
import { parseFileMode } from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "ext:deno_node/buffer.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function chmod(
  path: string | Buffer | URL,
  mode: string | number,
  callback: CallbackWithError,
) {
  path = getValidatedPath(path).toString();

  try {
    mode = parseFileMode(mode, "mode");
  } catch (error) {
    // TODO(PolarETech): Errors should not be ignored when denoFs.chmod is supported on Windows.
    // https://github.com/denoland/deno_std/issues/2916
    if (core.build.os === "windows") {
      mode = 0; // set dummy value to avoid type checking error at denoFs.chmod
    } else {
      throw error;
    }
  }

  denoFs.chmod(pathModule.toNamespacedPath(path), mode).catch((error) => {
    // Ignore NotSupportedError that occurs on windows
    // https://github.com/denoland/deno_std/issues/2995
    if (!(error instanceof Deno.errors.NotSupported)) {
      throw error;
    }
  }).then(
    () => callback(null),
    callback,
  );
}

export const chmodPromise = promisify(chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

export function chmodSync(path: string | URL, mode: string | number) {
  path = getValidatedPath(path).toString();

  try {
    mode = parseFileMode(mode, "mode");
  } catch (error) {
    // TODO(PolarETech): Errors should not be ignored when denoFs.chmodSync is supported on Windows.
    // https://github.com/denoland/deno_std/issues/2916
    if (core.build.os === "windows") {
      mode = 0; // set dummy value to avoid type checking error at denoFs.chmodSync
    } else {
      throw error;
    }
  }

  try {
    denoFs.chmodSync(pathModule.toNamespacedPath(path), mode);
  } catch (error) {
    // Ignore NotSupportedError that occurs on windows
    // https://github.com/denoland/deno_std/issues/2995
    if (!(error instanceof Deno.errors.NotSupported)) {
      throw error;
    }
  }
}
