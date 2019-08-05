// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export type OperatingSystem = "mac" | "win" | "linux";

export type Arch = "x64" | "arm64";

// Do not add unsupported platforms.
/** Build related information */
export interface BuildInfo {
  /** The CPU architecture. */
  arch: Arch;

  /** The operating system. */
  os: OperatingSystem;
}

// 'build' is injected by rollup.config.js at compile time.
export const build: BuildInfo = {
  // These string will be replaced by rollup
  /* eslint-disable-next-line @typescript-eslint/no-explicit-any */
  arch: `ROLLUP_REPLACE_ARCH` as any,
  /* eslint-disable-next-line @typescript-eslint/no-explicit-any */
  os: `ROLLUP_REPLACE_OS` as any
};

// TODO(kevinkassimo): deprecate Deno.platform
export const platform = build;
