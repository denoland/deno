// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const core = window.Deno.core;

  class DynamicLibrary {
    #rid;
    symbols = {};

    constructor(path, symbols) {
      this.#rid = core.opSync("op_ffi_load", { path, symbols });

      for (const symbol in symbols) {
        this.symbols[symbol] = (...parameters) =>
          core.opSync("op_ffi_call", { rid: this.#rid, symbol, parameters });
      }
    }

    close() {
      core.close(this.#rid);
    }
  }

  function dlopen(path, symbols) {
    const prefix = "Failed to execute 'dlopen'";
    webidl.requiredArguments(arguments.length, 2, { prefix });
    path = webidl.converters.USVString(path, {
      prefix,
      context: "Argument 1",
    });
    symbols = webidl.converters.DLSymbols(symbols, {
      prefix,
      context: "Argument 2",
    });

    return new DynamicLibrary(path, symbols);
  }

  window.__bootstrap.ffi = { dlopen };
})(this);
