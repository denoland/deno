"use strict";
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const { errors } = window.__bootstrap.errors;

  class FsWatcher {
    #rid = 0;

    constructor(paths, options) {
      const { recursive } = options;
      this.#rid = core.jsonOpSync("op_fs_events_open", { recursive, paths });
    }

    get rid() {
      return this.#rid;
    }

    async next() {
      try {
        return await core.jsonOpAsync("op_fs_events_poll", {
          rid: this.rid,
        });
      } catch (error) {
        if (error instanceof errors.BadResource) {
          return { value: undefined, done: true };
        } else if (error instanceof errors.Interrupted) {
          return { value: undefined, done: true };
        }
        throw error;
      }
    }

    return(value) {
      core.close(this.rid);
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
