// Copyright 2018-2025 the Deno authors. MIT license.

import { isWindows } from "ext:deno_node/_util/os.ts";
import { primordials } from "ext:core/mod.js";

const { SafeRegExp } = primordials;

export const SEP = isWindows ? "\\" : "/";
export const SEP_PATTERN = isWindows
  ? new SafeRegExp("[\\\\/]+")
  : new SafeRegExp("\/+");
