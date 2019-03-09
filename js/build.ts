// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Do not add unsupported platforms.
/** Build related information */
export interface BuildInfo {
  /** The operating system CPU architecture. */
  arch: "x64";

  /** The operating system platform. */
  os: "mac" | "win" | "linux";

  /** The GN build arguments */
  gnArgs: string;
}

// 'build' is injected by rollup.config.js at compile time.
export const build: BuildInfo = {
  /* eslint-disable @typescript-eslint/no-explicit-any */
  // These string will be replaced by rollup
  arch: `ROLLUP_REPLACE_ARCH` as any,
  os: `ROLLUP_REPLACE_OS` as any,
  gnArgs: `ROLLUP_REPLACE_GN_ARGS`
  /* eslint-enable @typescript-eslint/no-explicit-any */
};

// TODO(kevinkassimo): deprecate Deno.platform
export const platform = build;
