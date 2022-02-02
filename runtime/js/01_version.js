// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { ObjectFreeze } = window.__bootstrap.primordials;

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

    ObjectFreeze(version);
  }

  window.__bootstrap.version = {
    version,
    setVersions,
  };
})(this);
