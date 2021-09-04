// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { build } = window.__bootstrap.build;
  const { errors } = window.__bootstrap.errors;
  const {
    Error,
    Promise,
    PromisePrototypeThen,
    PromiseResolve,
    SymbolAsyncIterator,
  } = window.__bootstrap.primordials;

  function bindSignal(signo) {
    return core.opSync("op_signal_bind", signo);
  }

  function pollSignal(rid) {
    return core.opAsync("op_signal_poll", rid);
  }

  function unbindSignal(rid) {
    core.opSync("op_signal_unbind", rid);
  }

  function signal(signo) {
    if (build.os === "windows") {
      throw new Error("not implemented!");
    }
    return new SignalStream(signo);
  }

  class SignalStream {
    #disposed = false;
    #pollingPromise = PromiseResolve(false);
    #rid = 0;

    constructor(signo) {
      this.#rid = bindSignal(signo);
      this.#loop();
    }

    #pollSignal = async () => {
      let done;
      try {
        done = await pollSignal(this.#rid);
      } catch (error) {
        if (error instanceof errors.BadResource) {
          return true;
        }
        throw error;
      }
      return done;
    };

    #loop = async () => {
      do {
        this.#pollingPromise = this.#pollSignal();
      } while (!(await this.#pollingPromise) && !this.#disposed);
    };

    then(
      f,
      g,
    ) {
      const p = PromisePrototypeThen(this.#pollingPromise, (done) => {
        if (done) {
          // If pollingPromise returns true, then
          // this signal stream is finished and the promise API
          // should never be resolved.
          return new Promise(() => {});
        }
        return;
      });
      return PromisePrototypeThen(p, f, g);
    }

    async next() {
      return { done: await this.#pollingPromise, value: undefined };
    }

    [SymbolAsyncIterator]() {
      return this;
    }

    dispose() {
      if (this.#disposed) {
        throw new Error("The stream has already been disposed.");
      }
      this.#disposed = true;
      unbindSignal(this.#rid);
    }
  }

  window.__bootstrap.signals = {
    signal,
    SignalStream,
  };
})(this);
