// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { ObjectFreeze, StringPrototypeSplit } = window.__bootstrap.primordials;

  const build = {
    target: "unknown",
    arch: "unknown",
    os: "unknown",
    vendor: "unknown",
    env: undefined,
  };

  function setBuildInfo(target) {
    const [arch, vendor, os, env] = StringPrototypeSplit(target, "-", 4);
    build.target = target;
    build.arch = arch;
    build.vendor = vendor;
    build.os = os;
    build.env = env;
    ObjectFreeze(build);
  }

  window.__bootstrap.build = {
    build,
    setBuildInfo,
  };
})(this);
