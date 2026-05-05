// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const { op_node_build_os } = core.ops;

type OSType =
  | "windows"
  | "linux"
  | "android"
  | "darwin"
  | "freebsd"
  | "openbsd";

const osType: OSType = op_node_build_os();

const isAndroid = osType === "android";
const isWindows = osType === "windows";
const isLinux = osType === "linux" || osType === "android";
const isMacOS = osType === "darwin";

return { osType, isAndroid, isWindows, isLinux, isMacOS };
})();
