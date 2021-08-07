// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const { stat, statSync, chmod, chmodSync } = window.__bootstrap.fs;
  const { open, openSync } = window.__bootstrap.files;
  const { build } = window.__bootstrap.build;
  const {
    TypedArrayPrototypeSubarray,
  } = window.__bootstrap.primordials;

  function writeFileSync(
    path,
    data,
    options = {},
  ) {
    if (options?.signal?.aborted) {
      throw new DOMException("The write operation was aborted.", "AbortError");
    }
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        statSync(path);
      }
    }

    const openOptions = options.append
      ? { write: true, create: true, append: true }
      : { write: true, create: true, truncate: true };
    const file = openSync(path, openOptions);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "windows"
    ) {
      chmodSync(path, options.mode);
    }

    let nwritten = 0;
    while (nwritten < data.length) {
      nwritten += file.writeSync(TypedArrayPrototypeSubarray(data, nwritten));
    }

    file.close();
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
    while (!signal?.aborted && nwritten < data.length) {
      nwritten += await file.write(TypedArrayPrototypeSubarray(data, nwritten));
    }

    file.close();

    if (signal?.aborted) {
      throw new DOMException("The write operation was aborted.", "AbortError");
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
