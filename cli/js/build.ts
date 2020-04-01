// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type OperatingSystem = "mac" | "win" | "linux";

export type Arch = "x64" | "arm64";

// Do not add unsupported platforms.
export interface BuildInfo {
  arch: Arch;

  os: OperatingSystem;
}

export const build: BuildInfo = {
  arch: "" as Arch,
  os: "" as OperatingSystem,
};

export function setBuildInfo(os: OperatingSystem, arch: Arch): void {
  build.os = os;
  build.arch = arch;

  Object.freeze(build);
}
