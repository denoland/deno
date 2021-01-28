// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const version = {
    deno: "",
    v8: "",
    typescript: "",
  };

  function setVersions(
    denoVersion,
    v8Version,
    tsVersion,
  ) {
    version.deno = denoVersion;
    version.v8 = v8Version;
    version.typescript = tsVersion;

    Object.freeze(version);
  }

  window.__bootstrap.version = {
    version,
    setVersions,
  };
})(this);
