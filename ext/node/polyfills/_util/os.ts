// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const { ops } = globalThis.__bootstrap.core;

export type OSType = "windows" | "linux" | "darwin" | "freebsd";

export const osType: OSType = ops.op_node_build_os();

export const isWindows = osType === "windows";
export const isLinux = osType === "linux";
