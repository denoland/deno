// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const build = {
    target: "unknown",
    arch: "unknown",
    os: "unknown",
    vendor: "unknown",
    env: undefined,
  };

  function setBuildInfo(target) {
    const [arch, vendor, os, env] = target.split("-", 4);
    build.target = target;
    build.arch = arch;
    build.vendor = vendor;
    build.os = os;
    build.env = env;
    Object.freeze(build);
  }

  window.__bootstrap.build = {
    build,
    setBuildInfo,
  };
})(this);
