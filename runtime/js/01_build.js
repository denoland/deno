// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import primordials from "deno:core/00_primordials.js";
const { ObjectFreeze, StringPrototypeSplit } = primordials;

const build = {
  target: "unknown",
  arch: "unknown",
  os: "unknown",
  vendor: "unknown",
  env: undefined,
};

function setBuildInfo(target) {
  const { 0: arch, 1: vendor, 2: os, 3: env } = StringPrototypeSplit(
    target,
    "-",
    4,
  );
  build.target = target;
  build.arch = arch;
  build.vendor = vendor;
  build.os = os;
  build.env = env;
  ObjectFreeze(build);
}

export { build, setBuildInfo };
