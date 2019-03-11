// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Do not add unsupported platforms.
/** Build related information */
export interface BuildInfo {
  /** The operating system CPU architecture. */
  arch: "x64";

  /** The operating system. */
  os: OperatingSystem;

  /** The arguments passed to GN during build. See `gn help buildargs`. */
  args: string;
}

/** The operating system platform. */
export type OperatingSystem = "mac" | "win" | "linux";

// 'build' is injected by rollup.config.js at compile time.
export const build: BuildInfo = {
  // These string will be replaced by rollup
  /* eslint-disable-next-line @typescript-eslint/no-explicit-any */
  arch: `ROLLUP_REPLACE_ARCH` as any,
  os: `ROLLUP_REPLACE_OS` as any,
  /* eslint-enable @typescript-eslint/no-explicit-any */
  args: `ROLLUP_REPLACE_GN_ARGS`
};

// TODO(kevinkassimo): deprecate Deno.platform
export const platform = build;
