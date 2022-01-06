// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { open, openSync } = window.__bootstrap.files;
  const { readAllSync, readAll, readAllSyncSized, readAllInnerSized } =
    window.__bootstrap.io;

  function readFileSync(path) {
    const file = openSync(path);
    try {
      const { size } = file.statSync();
      if (size === 0) {
        return readAllSync(file);
      } else {
        return readAllSyncSized(file, size);
      }
    } finally {
      file.close();
    }
  }

  async function readFile(path, options) {
    const file = await open(path);
    try {
      const { size } = await file.stat();
      if (size === 0) {
        return await readAll(file);
      } else {
        return await readAllInnerSized(file, size, options);
      }
    } finally {
      file.close();
    }
  }

  function readTextFileSync(path) {
    return core.decode(readFileSync(path));
  }

  async function readTextFile(path, options) {
    return core.decode(await readFile(path, options));
  }

  window.__bootstrap.readFile = {
    readFile,
    readFileSync,
    readTextFileSync,
    readTextFile,
  };
})(this);
