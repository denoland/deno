// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { setTarget } = window.__bootstrap.event;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayPrototypeIndexOf,
    ArrayPrototypeSplice,
    ArrayPrototypePush,
    Symbol,
    Uint8Array,
    ObjectDefineProperty,
    Map,
    MapPrototypeSet,
    MapPrototypeGet,
    FunctionPrototypeCall,
  } = window.__bootstrap.primordials;

  const handlerSymbol = Symbol("eventHandlers");
  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return FunctionPrototypeCall(wrappedHandler.handler, this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  // TODO(lucacasonato) reuse when we can reuse code between web crates
  function defineEventHandler(emitter, name) {
    // HTML specification section 8.1.5.1
    ObjectDefineProperty(emitter, `on${name}`, {
      get() {
        // TODO(bnoordhuis) The "BroadcastChannel should have an onmessage
        // event" WPT test expects that .onmessage !== undefined. Returning
        // null makes it pass but is perhaps not exactly in the spirit.
        if (!this[handlerSymbol]) {
          return null;
        }
        return MapPrototypeGet(this[handlerSymbol], name)?.handler ?? null;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = MapPrototypeGet(this[handlerSymbol], name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
        }
        MapPrototypeSet(this[handlerSymbol], name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }

  const _name = Symbol("[[name]]");
  const _closed = Symbol("[[closed]]");

  const channels = [];
  let rid = null;

  async function recv() {
    while (channels.length > 0) {
      const message = await core.opAsync("op_broadcast_recv", rid);

      if (message === null) {
        break;
      }

      const [name, data] = message;
      dispatch(null, name, new Uint8Array(data));
    }

    core.close(rid);
    rid = null;
  }

  function dispatch(source, name, data) {
    for (const channel of channels) {
      if (channel === source) continue; // Don't self-send.
      if (channel[_name] !== name) continue;
      if (channel[_closed]) continue;

      const go = () => {
        if (channel[_closed]) return;
        const event = new MessageEvent("message", {
          data: core.deserialize(data), // TODO(bnoordhuis) Cache immutables.
          origin: "http://127.0.0.1",
        });
        setTarget(event, channel);
        channel.dispatchEvent(event);
      };

      defer(go);
    }
  }

  // Defer to avoid starving the event loop. Not using queueMicrotask()
  // for that reason: it lets promises make forward progress but can
  // still starve other parts of the event loop.
  function defer(go) {
    setTimeout(go, 1);
  }

  class BroadcastChannel extends EventTarget {
    [_name];
    [_closed] = false;

    get name() {
      return this[_name];
    }

    constructor(name) {
      super();

      const prefix = "Failed to construct 'BroadcastChannel'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      this[_name] = webidl.converters["DOMString"](name, {
        prefix,
        context: "Argument 1",
      });

      this[webidl.brand] = webidl.brand;

      ArrayPrototypePush(channels, this);

      if (rid === null) {
        // Create the rid immediately, otherwise there is a time window (and a
        // race condition) where messages can get lost, because recv() is async.
        rid = core.opSync("op_broadcast_subscribe");
        recv();
      }
    }

    postMessage(message) {
      webidl.assertBranded(this, BroadcastChannel);

      const prefix = "Failed to execute 'postMessage' on 'BroadcastChannel'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      if (this[_closed]) {
        throw new DOMException("Already closed", "InvalidStateError");
      }

      if (typeof message === "function" || typeof message === "symbol") {
        throw new DOMException("Uncloneable value", "DataCloneError");
      }

      const data = core.serialize(message);

      // Send to other listeners in this VM.
      dispatch(this, this[_name], new Uint8Array(data));

      // Send to listeners in other VMs.
      defer(() => core.opAsync("op_broadcast_send", [rid, this[_name]], data));
    }

    close() {
      webidl.assertBranded(this, BroadcastChannel);
      this[_closed] = true;

      const index = ArrayPrototypeIndexOf(channels, this);
      if (index === -1) return;

      ArrayPrototypeSplice(channels, index, 1);
      if (channels.length === 0) core.opSync("op_broadcast_unsubscribe", rid);
    }
  }

  defineEventHandler(BroadcastChannel.prototype, "message");
  defineEventHandler(BroadcastChannel.prototype, "messageerror");

  window.__bootstrap.broadcastChannel = { BroadcastChannel };
})(this);
