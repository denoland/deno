// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

// Keep this up-to-date with Deno.build.os
export type OSType =
  | "darwin"
  | "linux"
  | "windows"
  | "freebsd"
  | "netbsd"
  | "aix"
  | "solaris"
  | "illumos";

export const osType: OSType = (() => {
  // deno-lint-ignore no-explicit-any
  const { Deno } = globalThis as any;
  if (typeof Deno?.build?.os === "string") {
    return Deno.build.os;
  }

  // deno-lint-ignore no-explicit-any
  const { navigator } = globalThis as any;
  if (navigator?.appVersion?.includes?.("Win")) {
    return "windows";
  }

  return "linux";
})();

export const isWindows = osType === "windows";
