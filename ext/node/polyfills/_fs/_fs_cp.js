// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file prefer-primordials

import {
  getValidatedPath,
  validateCpOptions,
} from "ext:deno_node/internal/fs/utils.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";

const core = globalThis.__bootstrap.core;
const ops = core.ops;
const { op_node_cp } = core.ensureFastOps();

export function cpSync(src, dest, options) {
  validateCpOptions(options);
  const srcPath = getValidatedPath(src, "src");
  const destPath = getValidatedPath(dest, "dest");

  ops.op_node_cp_sync(srcPath, destPath);
}

export function cp(src, dest, options, callback) {
  if (typeof options === "function") {
    callback = options;
    options = {};
  }
  validateCpOptions(options);
  const srcPath = getValidatedPath(src, "src");
  const destPath = getValidatedPath(dest, "dest");

  op_node_cp(
    srcPath,
    destPath,
  ).then(
    (res) => callback(null, res),
    (err) => callback(err, null),
  );
}

export const cpPromise = promisify(cp);
