// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

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
  // TODO(lucacasonato) reuse when we can reuse code between web crates
  function defineEventHandler(emitter, name) {
    // HTML specification section 8.1.5.1
    Object.defineProperty(emitter, `on${name}`, {
      get() {
        return this[handlerSymbol]?.get(name)?.handler;
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

  const _name = Symbol("[[name]]");
  const _closed = Symbol("[[closed]]");
  const _rid = Symbol("[[rid]]");

  class BroadcastChannel extends EventTarget {
    [_name];
    [_closed] = false;
    [_rid];

    get name() {
      return this[_name];
    }

    constructor(name) {
      super();

      window.location;

      const prefix = "Failed to construct 'broadcastChannel'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      this[_name] = webidl.converters["DOMString"](name, {
        prefix,
        context: "Argument 1",
      });

      this[_rid] = core.opSync("op_broadcast_open", this[_name]);

      this[webidl.brand] = webidl.brand;

      this.#eventLoop();
    }

    postMessage(message) {
      webidl.assertBranded(this, BroadcastChannel);

      if (this[_closed]) {
        throw new DOMException("Already closed", "InvalidStateError");
      }

      core.opAsync("op_broadcast_send", this[_rid], core.serialize(message));
    }

    close() {
      webidl.assertBranded(this, BroadcastChannel);

      this[_closed] = true;
      core.close(this[_rid]);
    }

    async #eventLoop() {
      while (!this[_closed]) {
        const message = await core.opAsync(
          "op_broadcast_next_event",
          this[_rid],
        );

        if (message.length !== 0) {
          const event = new MessageEvent("message", {
            data: core.deserialize(message),
            origin: window.location,
          });
          event.target = this;
          this.dispatchEvent(event);
        }
      }
    }
  }

  defineEventHandler(BroadcastChannel.prototype, "message");
  defineEventHandler(BroadcastChannel.prototype, "messageerror");

  window.__bootstrap.broadcastChannel = { BroadcastChannel };
})(this);
