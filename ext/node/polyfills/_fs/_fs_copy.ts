// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
const { makeCallback } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_common.ts",
);
import type { Buffer } from "node:buffer";
import {
  getValidatedPathToString,
  getValidMode,
} from "ext:deno_node/internal/fs/utils.mjs";

const {
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
} = primordials;
const { fs } = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
);
const { UV_EEXIST } = core.loadExtScript(
  "ext:deno_node/internal_binding/uv.ts",
);
const { denoErrorToNodeError, uvException } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");

export function copyFile(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  callback: CallbackWithError,
): void;
export function copyFile(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode: number,
  callback: CallbackWithError,
): void;
export function copyFile(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode: number | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (typeof mode === "function") {
    callback = mode;
    mode = 0;
  }
  const srcStr = getValidatedPathToString(src, "src");
  const destStr = getValidatedPathToString(dest, "dest");
  const modeNum = getValidMode(mode, "copyFile");
  const cb = makeCallback(callback);
  const ctx = { syscall: "copyfile", path: srcStr, dest: destStr };
  const onError = (e: Error) => cb(denoErrorToNodeError(e, ctx));
  const copy = () => {
    PromisePrototypeThen(
      Deno.copyFile(srcStr, destStr),
      () => cb(null),
      onError,
    );
  };

  if ((modeNum & fs.COPYFILE_EXCL) === fs.COPYFILE_EXCL) {
    PromisePrototypeThen(Deno.lstat(destStr), () => {
      cb(uvException({ errno: UV_EEXIST, ...ctx }));
    }, (e: Error) => {
      if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, e)) {
        copy();
      } else {
        onError(e);
      }
    });
  } else {
    copy();
  }
}

export const copyFilePromise = promisify(copyFile) as (
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

export function copyFileSync(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode?: number,
) {
  const srcStr = getValidatedPathToString(src, "src");
  const destStr = getValidatedPathToString(dest, "dest");
  const modeNum = getValidMode(mode, "copyFile");
  const ctx = { syscall: "copyfile", path: srcStr, dest: destStr };

  if ((modeNum & fs.COPYFILE_EXCL) === fs.COPYFILE_EXCL) {
    let destStat;
    try {
      destStat = Deno.lstatSync(destStr);
    } catch (e) {
      if (!ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, e)) {
        throw denoErrorToNodeError(e, ctx);
      }
    }
    if (destStat) {
      throw uvException({ errno: UV_EEXIST, ...ctx });
    }
  }

  try {
    Deno.copyFileSync(srcStr, destStr);
  } catch (e) {
    throw denoErrorToNodeError(e, ctx);
  }
}
