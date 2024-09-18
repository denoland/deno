// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_fs_events_open, op_fs_events_poll } from "ext:core/ops";
const {
  BadResourcePrototype,
  InterruptedPrototype,
} = core;
const {
  ArrayIsArray,
  ObjectPrototypeIsPrototypeOf,
  PromiseResolve,
  SymbolAsyncIterator,
} = primordials;

import { SymbolDispose } from "ext:deno_web/00_infra.js";

class FsWatcher {
  #rid = 0;
  #promise;

  constructor(paths, options) {
    const { recursive } = options;
    this.#rid = op_fs_events_open({ recursive, paths });
  }

  unref() {
    core.unrefOpPromise(this.#promise);
  }

  ref() {
    core.refOpPromise(this.#promise);
  }

  async next() {
    try {
      this.#promise = op_fs_events_poll(this.#rid);
      const value = await this.#promise;
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

  return(value) {
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
  options = { __proto__: null, recursive: true },
) {
  return new FsWatcher(ArrayIsArray(paths) ? paths : [paths], options);
}

export { watchFs };
