// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { BadResourcePrototype, InterruptedPrototype } = core;
  const {
    ArrayIsArray,
    ObjectPrototypeIsPrototypeOf,
    PromiseResolve,
    SymbolAsyncIterator,
  } = window.__bootstrap.primordials;
  class FsWatcher {
    #rid = 0;

    constructor(paths, options) {
      const { recursive } = options;
      this.#rid = ops.op_fs_events_open({ recursive, paths });
    }

    get rid() {
      return this.#rid;
    }

    async next() {
      try {
        const value = await core.opAsync("op_fs_events_poll", this.rid);
        return value
          ? { value, done: false }
          : { value: undefined, done: true };
      } catch (error) {
        if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
          return { value: undefined, done: true };
        } else if (
          ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)
        ) {
          return { value: undefined, done: true };
        }
        throw error;
      }
    }

    // TODO(kt3k): This is deprecated. Will be removed in v2.0.
    // See https://github.com/denoland/deno/issues/10577 for details
    return(value) {
      core.close(this.rid);
      return PromiseResolve({ value, done: true });
    }

    close() {
      core.close(this.rid);
    }

    [SymbolAsyncIterator]() {
      return this;
    }
  }

  function watchFs(
    paths,
    options = { recursive: true },
  ) {
    return new FsWatcher(ArrayIsArray(paths) ? paths : [paths], options);
  }

  window.__bootstrap.fsEvents = {
    watchFs,
  };
})(this);
