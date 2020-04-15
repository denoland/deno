// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/build.ts", [], function (exports_8, context_8) {
  "use strict";
  let build;
  const __moduleName = context_8 && context_8.id;
  function setBuildInfo(os, arch) {
    build.os = os;
    build.arch = arch;
    Object.freeze(build);
  }
  exports_8("setBuildInfo", setBuildInfo);
  return {
    setters: [],
    execute: function () {
      exports_8(
        "build",
        (build = {
          arch: "",
          os: "",
        })
      );
    },
  };
});
