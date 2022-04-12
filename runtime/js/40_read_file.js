// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function readFileSync(path) {
    return core.opSync("op_readfile_sync", path);
  }

  function readFile(path, _options) {
    return core.opAsync("op_readfile_async", path);
  }

  function readTextFileSync(path) {
    return core.opSync("op_readfile_text_sync", path);
  }

  function readTextFile(path, _options) {
    return core.opAsync("op_readfile_text_async", path);
  }

  window.__bootstrap.readFile = {
    readFile,
    readFileSync,
    readTextFileSync,
    readTextFile,
  };
})(this);
