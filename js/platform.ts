// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not add unsupported platforms.
export interface Platform {
  /** The operating system CPU architecture. */
  arch: "x64";

  /** The operating system platform. */
  os: "mac" | "win" | "linux";
}

// 'platform' is  injected by rollup.config.js at compile time.
export const platform: Platform = {
  // tslint:disable:no-any
  arch: "" as any,
  os: "" as any
  // tslint:disable:any
};
