// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const {
  BadResourcePrototype,
  InterruptedPrototype,
} = core;
const {
  op_fs_events_open,
  op_fs_events_poll,
} = core.ensureFastOps();
const {
  ArrayIsArray,
  ObjectPrototypeIsPrototypeOf,
  PromiseResolve,
  SymbolAsyncIterator,
} = primordials;

import { SymbolDispose } from "ext:deno_web/00_infra.js";

class FsWatcher {
  #rid = 0;

  constructor(paths, options) {
    const { recursive } = options;
    this.#rid = op_fs_events_open({ recursive, paths });
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.FsWatcher.rid",
      new Error().stack,
      "Use `Deno.FsWatcher` instance methods instead.",
    );
    return this.#rid;
  }

  async next() {
    try {
      const value = await op_fs_events_poll(this.#rid);
      return value ? { value, done: false } : { value: undefined, done: true };
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
    internals.warnOnDeprecatedApi("Deno.FsWatcher.return()", new Error().stack);
    core.close(this.#rid);
    return PromiseResolve({ value, done: true });
  }

  close() {
    core.close(this.#rid);
  }

  [SymbolAsyncIterator]() {
    return this;
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}

function watchFs(
  paths,
  options = { recursive: true },
) {
  return new FsWatcher(ArrayIsArray(paths) ? paths : [paths], options);
}

export { watchFs };
