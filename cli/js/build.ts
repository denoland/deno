// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export const build = {
  target: "unknown",
  arch: "unknown",
  os: "unknown",
  vendor: "unknown",
  env: undefined as string | undefined,
};

export function setBuildInfo(target: string): void {
  const [arch, vendor, os, env] = target.split("-", 4);
  build.target = target;
  build.arch = arch;
  build.vendor = vendor;
  build.os = os;
  build.env = env;
  Object.freeze(build);
}
