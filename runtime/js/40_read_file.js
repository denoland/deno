// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;

  function readFileSync(path) {
    return core.opSync("op_readfile_sync", pathFromURL(path));
  }

  function readFile(path, options) {
    return core.opAsync(
      "op_readfile_async",
      pathFromURL(path),
      options?.signal ?? null,
    );
  }

  function readTextFileSync(path) {
    return core.opSync("op_readfile_text_sync", pathFromURL(path));
  }

  function readTextFile(path, options) {
    return core.opAsync(
      "op_readfile_text_async",
      pathFromURL(path),
      options?.signal ?? null,
    );
  }

  window.__bootstrap.readFile = {
    readFile,
    readFileSync,
    readTextFileSync,
    readTextFile,
  };
})(this);
