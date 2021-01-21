// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  class DyLibaray {
    #rid = 0;
    constructor(rid) {
      this.#rid = rid;
    }

    call(name, options) {
      const { params = [], returnType = "" } = options ?? {};
      return core.jsonOpSync("op_call_libaray_ffi", {
        rid: this.#rid,
        name,
        params,
        returnType,
      });
    }

    close() {
      core.close(this.#rid);
    }
  }

  function loadLibrary(filename) {
    const rid = core.jsonOpSync("op_load_libaray", { filename });
    return new DyLibaray(rid);
  }

  window.__bootstrap.ffi = {
    loadLibrary,
  };
})(this);
