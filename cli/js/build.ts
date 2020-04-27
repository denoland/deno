// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//@ts-ignore
export const build: {
    target: string,
    arch: string,
    os: string,
    vendor: string,
    env?: string,
}= {};

export function setBuildInfo(target: string): void {
  const [arch, vendor, os, env] = target.split("-", 4);
  //@ts-ignore
  build.target = target;
  //@ts-ignore
  build.arch = arch;
  //@ts-ignore
  build.vendor = vendor;
  //@ts-ignore
  build.os = os;
  //@ts-ignore
  build.env = env;
  Object.freeze(build);
}
