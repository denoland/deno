// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const core = window.__bootstrap.core;
  const { abortSignal } = window.__bootstrap;
  const { pathFromURL } = window.__bootstrap.util;
  const { openSync } = window.__bootstrap.files;

  function writeFileSync(
    path,
    data,
    options = {},
  ) {
    options.signal?.throwIfAborted();
    core.opSync("op_write_file_sync", {
      path: pathFromURL(path),
      data,
      mode: options.mode,
      append: options.append ?? false,
      create: options.create ?? true,
    });
  }

  const openFdCache = new Map();
  const registry = new FinalizationRegistry(({ fsFile, path }) => {
    core.tryClose(fsFile.rid);
    openFdCache.delete(path);
  });

  function invalidateFdCache(fsFile, create, append, mode) {
    if (!fsFile || fsFile.mode || mode) return true;
    return fsFile.append !== append || fsFile.create !== create;
  }

  async function writeFile(
    pathOrURL,
    data,
    options = {},
  ) {
    let cancelRid;
    let abortHandler;
    if (options.signal) {
      options.signal.throwIfAborted();
      cancelRid = core.opSync("op_cancel_handle");
      abortHandler = () => core.tryClose(cancelRid);
      options.signal[abortSignal.add](abortHandler);
    }
    const path = pathFromURL(pathOrURL);
    let fsFile = openFdCache.get(path);
    const append = options.append ?? false;
    const create = options.create ?? true;
    if (invalidateFdCache(fsFile, create, append, options.mode)) {
      const fd = openSync(path, {
        mode: options.mode,
        read: false,
        write: true,
        create,
        truncate: !append,
        append,
        createNew: false,
      });
      fsFile = { rid: fd.rid, append, create, mode: options.mode };
      if (options.mode === undefined && openFdCache.size < 20) openFdCache.set(path, fsFile);
    }
    try {
      registry.register({ fsFile, path });
      const len = data.byteLength;
      let written = 0;
      while (written !== len) {
        const n = await core.write(
          fsFile.rid,
          written ? data.slice(written) : data,
        );
        written += n;
        if (options.signal) options.signal.throwIfAborted();
      }
    } finally {
      if (options.signal) {
        options.signal[abortSignal.remove](abortHandler);

        // always throw the abort error when aborted
        options.signal.throwIfAborted();
      }
    }
  }

  function writeTextFileSync(
    path,
    data,
    options = {},
  ) {
    return writeFileSync(path, data, options);
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
