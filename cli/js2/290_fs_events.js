// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__dispatchJson;
  const { close } = window.__resources;

  class FsWatcher {
    #rid = 0;

    constructor(paths, options) {
      const { recursive } = options;
      this.#rid = sendSync("op_fs_events_open", { recursive, paths });
    }

    get rid() {
      return this.#rid;
    }

    next() {
      return sendAsync("op_fs_events_poll", {
        rid: this.rid,
      });
    }

    return(value) {
      close(this.rid);
      return Promise.resolve({ value, done: true });
    }

    [Symbol.asyncIterator]() {
      return this;
    }
  }

  function watchFs(
    paths,
    options = { recursive: true },
  ) {
    return new FsWatcher(Array.isArray(paths) ? paths : [paths], options);
  }

  window.__fsEvents = {
    watchFs,
  };
})(this);
