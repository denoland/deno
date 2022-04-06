// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const core = window.__bootstrap.core;
  const { stat, chmod } = window.__bootstrap.fs;
  const { open } = window.__bootstrap.files;
  const { build } = window.__bootstrap.build;
  const {
    TypedArrayPrototypeSubarray,
  } = window.__bootstrap.primordials;

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
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        await stat(path);
      }
    }

    const openOptions = options.append
      ? { write: true, create: true, append: true }
      : { write: true, create: true, truncate: true };
    const file = await open(path, openOptions);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "windows"
    ) {
      await chmod(path, options.mode);
    }

    const signal = options?.signal ?? null;
    let nwritten = 0;
    try {
      while (nwritten < data.length) {
        signal?.throwIfAborted();
        nwritten += await file.write(
          TypedArrayPrototypeSubarray(data, nwritten),
        );
      }
    } finally {
      file.close();
    }
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
