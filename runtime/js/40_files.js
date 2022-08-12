// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { read, readSync, write, writeSync } = window.__bootstrap.io;
  const { ftruncate, ftruncateSync, fstat, fstatSync } = window.__bootstrap.fs;
  const { pathFromURL } = window.__bootstrap.util;
  const { writableStreamForRid } = window.__bootstrap.streamUtils;
  const { readableStreamForRid } = window.__bootstrap.streams;
  const {
    ArrayPrototypeFilter,
    Error,
    ObjectValues,
  } = window.__bootstrap.primordials;

  function seekSync(
    rid,
    offset,
    whence,
  ) {
    return ops.op_seek_sync({ rid, offset, whence });
  }

  function seek(
    rid,
    offset,
    whence,
  ) {
    return core.opAsync("op_seek_async", { rid, offset, whence });
  }

  function openSync(
    path,
    options = { read: true },
  ) {
    checkOpenOptions(options);
    const mode = options?.mode;
    const rid = ops.op_open_sync(
      { path: pathFromURL(path), options, mode },
    );

    return new FsFile(rid);
  }

  async function open(
    path,
    options = { read: true },
  ) {
    checkOpenOptions(options);
    const mode = options?.mode;
    const rid = await core.opAsync(
      "op_open_async",
      { path: pathFromURL(path), options, mode },
    );

    return new FsFile(rid);
  }

  function createSync(path) {
    return openSync(path, {
      read: true,
      write: true,
      truncate: true,
      create: true,
    });
  }

  function create(path) {
    return open(path, {
      read: true,
      write: true,
      truncate: true,
      create: true,
    });
  }

  class FsFile {
    #rid = 0;

    #readable;
    #writable;

    constructor(rid) {
      this.#rid = rid;
    }

    get rid() {
      return this.#rid;
    }

    write(p) {
      return write(this.rid, p);
    }

    writeSync(p) {
      return writeSync(this.rid, p);
    }

    truncate(len) {
      return ftruncate(this.rid, len);
    }

    truncateSync(len) {
      return ftruncateSync(this.rid, len);
    }

    read(p) {
      return read(this.rid, p);
    }

    readSync(p) {
      return readSync(this.rid, p);
    }

    seek(offset, whence) {
      return seek(this.rid, offset, whence);
    }

    seekSync(offset, whence) {
      return seekSync(this.rid, offset, whence);
    }

    stat() {
      return fstat(this.rid);
    }

    statSync() {
      return fstatSync(this.rid);
    }

    close() {
      core.close(this.rid);
    }

    get readable() {
      if (this.#readable === undefined) {
        this.#readable = readableStreamForRid(this.rid);
      }
      return this.#readable;
    }

    get writable() {
      if (this.#writable === undefined) {
        this.#writable = writableStreamForRid(this.rid);
      }
      return this.#writable;
    }
  }

  class Stdin {
    #readable;

    constructor() {
    }

    get rid() {
      return 0;
    }

    read(p) {
      return read(this.rid, p);
    }

    readSync(p) {
      return readSync(this.rid, p);
    }

    close() {
      core.close(this.rid);
    }

    get readable() {
      if (this.#readable === undefined) {
        this.#readable = readableStreamForRid(this.rid);
      }
      return this.#readable;
    }
  }

  class Stdout {
    #writable;

    constructor() {
    }

    get rid() {
      return 1;
    }

    write(p) {
      return write(this.rid, p);
    }

    writeSync(p) {
      return writeSync(this.rid, p);
    }

    close() {
      core.close(this.rid);
    }

    get writable() {
      if (this.#writable === undefined) {
        this.#writable = writableStreamForRid(this.rid);
      }
      return this.#writable;
    }
  }

  class Stderr {
    #writable;

    constructor() {
    }

    get rid() {
      return 2;
    }

    write(p) {
      return write(this.rid, p);
    }

    writeSync(p) {
      return writeSync(this.rid, p);
    }

    close() {
      core.close(this.rid);
    }

    get writable() {
      if (this.#writable === undefined) {
        this.#writable = writableStreamForRid(this.rid);
      }
      return this.#writable;
    }
  }

  const stdin = new Stdin();
  const stdout = new Stdout();
  const stderr = new Stderr();

  function checkOpenOptions(options) {
    if (
      ArrayPrototypeFilter(
        ObjectValues(options),
        (val) => val === true,
      ).length === 0
    ) {
      throw new Error("OpenOptions requires at least one option to be true");
    }

    if (options.truncate && !options.write) {
      throw new Error("'truncate' option requires 'write' option");
    }

    const createOrCreateNewWithoutWriteOrAppend =
      (options.create || options.createNew) &&
      !(options.write || options.append);

    if (createOrCreateNewWithoutWriteOrAppend) {
      throw new Error(
        "'create' or 'createNew' options require 'write' or 'append' option",
      );
    }
  }

  window.__bootstrap.files = {
    stdin,
    stdout,
    stderr,
    File: FsFile,
    FsFile,
    create,
    createSync,
    open,
    openSync,
    seek,
    seekSync,
  };
})(this);
