// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  getValidatedPath,
  validateCpOptions,
} from "ext:deno_node/internal/fs/utils.mjs";
const core = globalThis.__bootstrap.core;
const ops = core.ops;

export function cpSync(src, dest, options) {
  validateCpOptions(options);
  const srcPath = getValidatedPath(src, "src");
  const destPath = getValidatedPath(dest, "dest");

  ops.op_node_cp_sync(srcPath, destPath);
}
