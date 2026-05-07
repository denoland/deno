// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_fs_exists, op_node_fs_exists_sync } from "ext:core/ops";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { core, primordials } from "ext:core/mod.js";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import type { Buffer } from "node:buffer";
const { kCustomPromisifiedSymbol } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
import * as process from "node:process";

const { ObjectDefineProperty, Promise, PromisePrototypeThen } = primordials;

type ExistsCallback = (exists: boolean) => void;

/**
 * Deprecated in node api
 */
export function exists(path: string | Buffer | URL, callback: ExistsCallback) {
  callback = makeCallback(callback);

  try {
    path = getValidatedPathToString(path);
  } catch {
    callback(false);
    return;
  }

  PromisePrototypeThen(
    op_node_fs_exists(path),
    callback,
  );
}

// The callback of fs.exists doesn't have standard callback signature.
// We need to provide special implementation for promisify.
// See https://github.com/nodejs/node/pull/13316
const existsPromisified = (path: string | URL) => {
  return new Promise((resolve) => {
    exists(path, (exists) => resolve(exists));
  });
};
// Rename so `promisify(fs.exists).name === 'exists'`, matching Node
// (see lib/fs.js which uses `function exists(path)` as the value).
ObjectDefineProperty(existsPromisified, "name", {
  __proto__: null,
  value: "exists",
  configurable: true,
});
ObjectDefineProperty(exists, kCustomPromisifiedSymbol, {
  __proto__: null,
  value: existsPromisified,
  enumerable: false,
  writable: false,
  configurable: true,
});

let showExistsDeprecation = true;
export function existsSync(path: string | Buffer | URL): boolean {
  try {
    path = getValidatedPathToString(path);
  } catch (err) {
    // @ts-expect-error `code` is safe to check with optional chaining
    if (showExistsDeprecation && err?.code === "ERR_INVALID_ARG_TYPE") {
      process.emitWarning(
        "Passing invalid argument types to fs.existsSync is deprecated",
        "DeprecationWarning",
        "DEP0187",
      );
      showExistsDeprecation = false;
    }
    return false;
  }
  return op_node_fs_exists_sync(path);
}
