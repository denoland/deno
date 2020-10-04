((window) => {
  const core = window.Deno.core;

  class Clipboard extends EventTarget {
    #rid;

    #getRid() {
      if (!this.#rid) {
      this.#rid = core.jsonOpSync("op_clipboard_create");
      }

      return this.#rid;
    }

    constructor() {
      super();
    }

    read() {
      throw new Error('Not yet implemented: only readText is supported');
    }

    readText() {
      return new Promise(resolve => {
        const data = core.jsonOpSync("op_clipboard_read", {rid: this.#getRid()});
        resolve(data);
      });
    }

    write() {
      throw new Error('Not yet implemented: only writeText is supported');
    }

    writeText(data) {
      return new Promise(resolve => {
        core.jsonOpSync("op_clipboard_write", {rid: this.#getRid(), content: data});
        resolve();
      });
    }
  }


  window.__bootstrap.clipboard = {
    Clipboard,
  }
})(this);
