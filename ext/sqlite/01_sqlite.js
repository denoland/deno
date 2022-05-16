// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  class Statement {
    #handle;
    constructor(conn, sql) {
      this.#handle = core.opSync("op_sqlite_prepare", conn, sql);
    }

    run(...args) {
      return core.opSync("op_sqlite_run", this.#handle, args);
    }

    query(...args) {
      return core.opSync("op_sqlite_query", this.#handle, args);
    }
  }

  class Connection {
    #rid;
    constructor(specifier, _flags) {
      this.#rid = core.opSync("op_sqlite_open", specifier);
    }

    prepare(sql) {
      return new Statement(this.#rid, sql);
    }
  }

  window.__bootstrap.sqlite = {
    Connection,
  };
})(this);
