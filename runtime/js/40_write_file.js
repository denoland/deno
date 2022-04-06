// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const core = window.__bootstrap.core;

  function writeFileSync(
    path,
    data,
    options = {},
  ) {
    options.signal?.throwIfAborted();
    core.opSync("op_write_file_sync", {
      path,
      data,
      mode: options.mode,
      append: options.append ?? false,
      create: options.create ?? true,
    });
  }

  async function writeFile(
    path,
    data,
    options = {},
  ) {
    options.signal?.throwIfAborted();
    // TODO(lucacasonato): support options.signal again
    await core.opAsync("op_write_file_async", {
      path,
      data,
      mode: options.mode,
      append: options.append ?? false,
      create: options.create ?? true,
    });
  }

  function writeTextFileSync(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFileSync(path, encoder.encode(data), options);
  }

  function writeTextFile(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFile(path, encoder.encode(data), options);
  }

  window.__bootstrap.writeFile = {
    writeTextFile,
    writeTextFileSync,
    writeFile,
    writeFileSync,
  };
})(this);
