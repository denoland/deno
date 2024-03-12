// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { BigInt, Number } from "ext:deno_node/internal/primordials.mjs";
import { op_node_statfs } from "ext:core/ops";
import { promisify } from "ext:deno_node/internal/util.mjs";

class StatFs {
  type;
  bsize;
  blocks;
  bfree;
  bavail;
  files;
  ffree;
  constructor(type, bsize, blocks, bfree, bavail, files, ffree) {
    this.type = type;
    this.bsize = bsize;
    this.blocks = blocks;
    this.bfree = bfree;
    this.bavail = bavail;
    this.files = files;
    this.ffree = ffree;
  }
}

export function statfs(path, options, callback) {
  if (typeof options === "function") {
    callback = options;
    options = {};
  }
  try {
    const res = statfsSync(path, options);
    callback(null, res);
  } catch (err) {
    callback(err, null);
  }
}

export function statfsSync(path, options): unknown {
  const bigint = typeof options?.bigint === "boolean" ? options.bigint : false;
  const statFs = op_node_statfs(
    path,
    bigint,
  );
  const parse = bigint ? BigInt : Number;
  return new StatFs(
    parse(statFs.type),
    parse(statFs.bsize),
    parse(statFs.blocks),
    parse(statFs.bfree),
    parse(statFs.bavail),
    parse(statFs.files),
    parse(statFs.ffree),
  );
}

export const statfsPromise = promisify(statfs);
