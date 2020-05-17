// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {

  const { sendSync } = window.__bootstrap.dispatchJson;

  function bufferStart(buffer) {
    // TODO how to implement this with op?
  }

  /*
    @param
      start: bigint
    @return SharedArrayBuffer
  */
  function bufferFromPointer(
    start,
    length
  ) {
    // TODO how to implement this with op?
  }


  class ForeignLibrary {
    #rid = 0;
    constructor(rid) {
      this.#rid = rid;
    }
    get rid() {
      return this.#rid;
    }

    lookup(symbol) {
      const addr = sendSync("op_dlsym", { rid, symbol });
      return BigInt(addr);
    }

    close() {
      close(this.rid);
    }
  }

  class ForeignFunction  {
    #rid = 0;
    constructor(rid) {
      this.#rid = rid;
    }
    get rid() {
      return this.#rid;
    }
    call(...args) {
      // TODO BigInt can't fit in JSON
      return sendSync("op_ffi_call", {
       rid,
       args,
      });
    }

    close() {
      close(this.rid);
    }
  }

  function loadForeignLibrary(path) {
    const rid = sendSync("op_ffi_dlopen", { path });
    return new ForeignLibrary(rid);
  }

  function loadForeignFunctionSync(
    addr,
    abi,
    info
  ) {
    return sendSync("op_ffi_prep", { addr, abi, ...info })
  }

  function listForeignAbiSync() {
    return sendSync("op_ffi_list_abi");
  }

  window.__bootstrap.ffiUnstable = {
    ForeignLibrary,
    ForeignFunction,
    loadForeignLibrary,
    loadForeignFunction,
    listForeignAbiSync,
    bufferStart,
    bufferFromPointer
  };
})(this);
