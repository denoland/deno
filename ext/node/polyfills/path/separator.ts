// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");

const { SafeRegExp } = primordials;

export const SEP = isWindows ? "\\" : "/";
export const SEP_PATTERN = isWindows
  ? new SafeRegExp("[\\\\/]+")
  : new SafeRegExp("\/+");
