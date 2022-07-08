// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const { TypeError } = window.__bootstrap.primordials;
  class Statement {
    #rid;
    #closed;

    constructor(conn, sql) {
      this.#rid = core.opSync("op_sqlite_prepare", conn, sql);
      this.#closed = false;
    }

    run(...args) {
      if (this.#closed) {
        throw new TypeError("Statement has already been disposed.");
      }
      return core.opSync("op_sqlite_run", this.#rid, args);
    }

    query(...args) {
      if (this.#closed) {
        throw new TypeError("Statement has already been disposed.");
      }
      return core.opSync("op_sqlite_query", this.#rid, args);
    }

    close() {
      if (this.#closed) {
        return;
      }
      core.close(this.#rid);
      this.#closed = true;
    }
  }

  class Connection {
    #rid;
    #statements;
    #closed;

    constructor(specifier, _flags) {
      this.#rid = core.opSync("op_sqlite_open", specifier);
      this.#statements = [];
      this.#closed = false;
    }

    prepare(sql) {
      if (this.#closed) {
        throw new TypeError("Connection has already been closed.");
      }
      const s = new Statement(this.#rid, sql);
      this.#statements.push(s);
      return s;
    }

    close() {
      if (this.#closed) {
        return;
      }

      for (const stmt of this.#statements) {
        stmt.close();
      }
      core.close(this.#rid);
      this.#closed = true;
    }
  }

  window.__bootstrap.sqlite = {
    Connection,
  };
})(this);
