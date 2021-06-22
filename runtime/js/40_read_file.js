// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { open, openSync } = window.__bootstrap.files;
  const { readAllInner, readAllSync } = window.__bootstrap.io;

  function readFileSync(path) {
    const file = openSync(path);
    try {
      const contents = readAllSync(file);
      return contents;
    } finally {
      file.close();
    }
  }

  async function readFile(path, options) {
    const file = await open(path);
    try {
      const contents = await readAllInner(file, options);
      return contents;
    } finally {
      file.close();
    }
  }

  function readTextFileSync(path) {
    const file = openSync(path);
    try {
      const contents = readAllSync(file);
      return core.decode(contents);
    } finally {
      file.close();
    }
  }

  async function readTextFile(path, options) {
    const file = await open(path);
    try {
      const contents = await readAllInner(file, options);
      return core.decode(contents);
    } finally {
      file.close();
    }
  }

  window.__bootstrap.readFile = {
    readFile,
    readFileSync,
    readTextFileSync,
    readTextFile,
  };
})(this);
