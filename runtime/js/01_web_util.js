// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const illegalConstructorKey = Symbol("illegalConstructorKey");

  function requiredArguments(
    name,
    length,
    required,
  ) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  const handlerSymbol = Symbol("eventHandlers");
  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return wrappedHandler.handler.call(this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  function defineEventHandler(emitter, name, defaultValue = undefined) {
    // HTML specification section 8.1.5.1
    Object.defineProperty(emitter, `on${name}`, {
      get() {
        return this[handlerSymbol]?.get(name)?.handler ?? defaultValue;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = this[handlerSymbol]?.get(name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
        }
        this[handlerSymbol].set(name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }
  window.__bootstrap.webUtil = {
    illegalConstructorKey,
    requiredArguments,
    defineEventHandler,
  };
})(this);
