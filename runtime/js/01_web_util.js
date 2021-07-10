// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    FunctionPrototypeCall,
    Map,
    MapPrototypeGet,
    MapPrototypeSet,
    ObjectDefineProperty,
    TypeError,
    Symbol,
  } = window.__bootstrap.primordials;
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
  function makeWrappedHandler(handler, isSpecialErrorEventHandler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      if (isSpecialErrorEventHandler) {
        const evt = args[0];
        if (evt instanceof ErrorEvent && evt.type === "error") {
          const ret = FunctionPrototypeCall(
            wrappedHandler.handler,
            this,
            evt.message,
            evt.filename,
            evt.lineno,
            evt.colno,
            evt.error,
          );
          if (ret === true) {
            evt.preventDefault();
          }
          return;
        }
      }

      return FunctionPrototypeCall(wrappedHandler.handler, this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  function defineEventHandler(
    emitter,
    name,
    defaultValue = undefined,
    isSpecialErrorEventHandler = false,
  ) {
    // HTML specification section 8.1.5.1
    ObjectDefineProperty(emitter, `on${name}`, {
      get() {
        if (!this[handlerSymbol]) {
          return defaultValue;
        }

        return MapPrototypeGet(this[handlerSymbol], name)?.handler ??
          defaultValue;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = MapPrototypeGet(this[handlerSymbol], name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(
            value,
            isSpecialErrorEventHandler,
          );
          this.addEventListener(name, handlerWrapper);
        }
        MapPrototypeSet(this[handlerSymbol], name, handlerWrapper);
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
