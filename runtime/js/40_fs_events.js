// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { BadResourcePrototype, InterruptedPrototype, ops } = core;
const {
  ArrayIsArray,
  ObjectPrototypeIsPrototypeOf,
  SymbolAsyncIterator,
} = primordials;
import { SymbolDispose } from "ext:deno_web/00_infra.js";
const {
  op_fs_events_poll,
} = core.ensureFastOps();

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
      const value = await op_fs_events_poll(this.rid);
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

  close() {
    core.close(this.rid);
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
