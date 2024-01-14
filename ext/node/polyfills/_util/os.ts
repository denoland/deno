// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const {
  op_node_build_os,
} = core.ensureFastOps(true);

export type OSType =
  | "windows"
  | "linux"
  | "android"
  | "darwin"
  | "freebsd"
  | "openbsd";

export const osType: OSType = op_node_build_os();

export const isWindows = osType === "windows";
export const isLinux = osType === "linux" || osType === "android";
