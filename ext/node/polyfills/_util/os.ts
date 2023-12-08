// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const ops = core.ops;

export type OSType = "windows" | "linux" | "darwin" | "freebsd" | "openbsd";

export const osType: OSType = ops.op_node_build_os();

export const isWindows = osType === "windows";
export const isLinux = osType === "linux";
