System.register("$deno$/version.ts", [], function (exports_11, context_11) {
  "use strict";
  let version;
  const __moduleName = context_11 && context_11.id;
  function setVersions(denoVersion, v8Version, tsVersion) {
    version.deno = denoVersion;
    version.v8 = v8Version;
    version.typescript = tsVersion;
    Object.freeze(version);
  }
  exports_11("setVersions", setVersions);
  return {
    setters: [],
    execute: function () {
      exports_11(
        "version",
        (version = {
          deno: "",
          v8: "",
          typescript: "",
        })
      );
    },
  };
});
