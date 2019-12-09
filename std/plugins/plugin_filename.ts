// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export type PluginFilenamePrefixType = "lib" | "";

export const pluginFilenamePrefix = ((): PluginFilenamePrefixType => {
  switch (Deno.build.os) {
    case "linux":
    case "mac":
      return "lib";
    case "win":
    default:
      return "";
  }
})();

export type PluginFilenameExtensionType = "so" | "dylib" | "dll";

export const pluginFilenameExtension = ((): PluginFilenameExtensionType => {
  switch (Deno.build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function pluginFilename(filenameBase: string): string {
  return pluginFilenamePrefix + filenameBase + "." + pluginFilenameExtension;
}
