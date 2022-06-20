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
    get rid() {
      return this.#rid;
    }

    constructor(key = null, rid) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }

      this.#rid = rid;
      this.readable = readableStreamForRid(rid);
      this.writable = writableStreamForRid(rid);
    }
  }

  function openPty({
    rows,
    columns,
  }) {
    const pty = core.opSync("op_pty_open", {
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
