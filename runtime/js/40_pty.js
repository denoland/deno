// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;
  const {
    ArrayPrototypeMap,
    ObjectEntries,
    String,
    TypeError,
    Uint8Array,
    PromiseAll,
  } = window.__bootstrap.primordials;
  const { readableStreamForRid, writableStreamForRid } =
    window.__bootstrap.streamUtils;

  class Pty {
    #rid;

    #pid;
    get pid() {
      return this.#pid;
    }

    constructor(key = null, { rid, pid }) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }

      this.#rid = rid;
      this.#pid = pid;

      this.readable = readableStreamForRid(rid);
      this.writable = writableStreamForRid(rid);
    }
  }

  function openPty(command, {
    args = [],
    cwd = undefined,
    clearEnv = false,
    env = {},
    rows,
    columns,
  }) {
    const pty = core.opSync("op_pty_open", {
      cmd: pathFromURL(command),
      args: ArrayPrototypeMap(args, String),
      cwd: pathFromURL(cwd),
      clearEnv,
      env: ObjectEntries(env),
      rows,
      columns,
    });
    return new Pty(illegalConstructorKey, pty);
  }

  window.__bootstrap.pty = {
    Pty,
    openPty,
  };
})(this);
