// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const { errors } = window.__bootstrap.errors;
  const { close } = window.__bootstrap.resources;

  class FsWatcher {
    #rid = 0;

    constructor(paths, options) {
      const { recursive } = options;
      this.#rid = sendSync("op_fs_events_open", { recursive, paths });
    }

    get rid() {
      return this.#rid;
    }

    async next() {
      try {
        return await sendAsync("op_fs_events_poll", {
          rid: this.rid,
        });
      } catch (error) {
        if (error instanceof errors.BadResource) {
          return { value: undefined, done: true };
        }
        throw error;
      }
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

  window.__bootstrap.fsEvents = {
    watchFs,
  };
})(this);
