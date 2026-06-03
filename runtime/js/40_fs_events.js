// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const { op_fs_events_open, op_fs_events_poll } = core.ops;
const {
  BadResourcePrototype,
  InterruptedPrototype,
} = core;
const {
  ArrayIsArray,
  ObjectPrototypeIsPrototypeOf,
  PromiseResolve,
  SymbolAsyncIterator,
  SymbolDispose,
} = primordials;

class FsWatcher {
  #rid = 0;
  #promise;
  #closed = false;

  constructor(paths, options) {
    const { recursive, ignore } = options;
    const ignorePaths = ignore === undefined
      ? []
      : (ArrayIsArray(ignore) ? ignore : [ignore]);
    this.#rid = op_fs_events_open(recursive, ignorePaths, paths);
  }

  unref() {
    core.unrefOpPromise(this.#promise);
  }

  ref() {
    core.refOpPromise(this.#promise);
  }

  async next() {
    if (this.#closed) {
      return { value: undefined, done: true };
    }
    try {
      this.#promise = op_fs_events_poll(this.#rid);
      const value = await this.#promise;
      return value ? { value, done: false } : { value: undefined, done: true };
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
        this.#closed = true;
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
    this.#close();
    return PromiseResolve({ value, done: true });
  }

  close() {
    this.#close();
  }

  [SymbolAsyncIterator]() {
    return this;
  }

  [SymbolDispose]() {
    this.#close();
  }

  #close() {
    if (!this.#closed) {
      this.#closed = true;
      core.tryClose(this.#rid);
    }
  }
}

function watchFs(
  paths,
  options = { __proto__: null, recursive: true },
) {
  return new FsWatcher(ArrayIsArray(paths) ? paths : [paths], options);
}

return { watchFs };
})();
