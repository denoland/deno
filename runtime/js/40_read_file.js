// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { open, openSync } = window.__bootstrap.files;
  const { readAll, readAllSync } = window.__bootstrap.io;

  function readFileSync(path) {
    const file = openSync(path);
    try {
      const contents = readAllSync(file);
      return contents;
    } finally {
      file.close();
    }
  }

  async function readFile(path) {
    const file = await open(path);
    try {
      const contents = await readAll(file);
      return contents;
    } finally {
      file.close();
    }
  }

  function readTextFileSync(path) {
    const file = openSync(path);
    try {
      const contents = readAllSync(file);
      const decoder = new TextDecoder();
      return decoder.decode(contents);
    } finally {
      file.close();
    }
  }

  async function readTextFile(path) {
    const file = await open(path);
    try {
      const contents = await readAll(file);
      const decoder = new TextDecoder();
      return decoder.decode(contents);
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
