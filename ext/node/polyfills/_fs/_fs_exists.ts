// Copyright 2018-2025 the Deno authors. MIT license.

import { op_node_fs_exists, op_node_fs_exists_sync } from "ext:core/ops";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { primordials } from "ext:core/mod.js";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import type { Buffer } from "node:buffer";
import * as pathModule from "node:path";
import { kCustomPromisifiedSymbol } from "ext:deno_node/internal/util.mjs";

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
    op_node_fs_exists(pathModule.toNamespacedPath(path)),
    callback,
  );
}

// The callback of fs.exists doesn't have standard callback signature.
// We need to provide special implementation for promisify.
// See https://github.com/nodejs/node/pull/13316
ObjectDefineProperty(exists, kCustomPromisifiedSymbol, {
  __proto__: null,
  value: (path: string | URL) => {
    return new Promise((resolve) => {
      exists(path, (exists) => resolve(exists));
    });
  },
  enumerable: false,
  writable: false,
  configurable: true,
});

export function existsSync(path: string | Buffer | URL): boolean {
  try {
    path = getValidatedPathToString(path);
  } catch {
    return false;
  }
  return op_node_fs_exists_sync(pathModule.toNamespacedPath(path));
}
