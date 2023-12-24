// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import {
  defineEventHandler,
  EventTarget,
  setIsTrusted,
  setTarget,
} from "ext:deno_web/02_event.js";
import { defer } from "ext:deno_web/02_timers.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
const {
  op_broadcast_recv,
  op_broadcast_send,
} = core.ensureFastOps();
const {
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  Uint8Array,
} = primordials;

const _name = Symbol("[[name]]");
const _closed = Symbol("[[closed]]");

const channels = [];
let rid = null;

async function recv() {
  while (channels.length > 0) {
    const message = await op_broadcast_recv(rid);

    if (message === null) {
      break;
    }

    const { 0: name, 1: data } = message;
    dispatch(null, name, new Uint8Array(data));
  }

  core.close(rid);
  rid = null;
}

function dispatch(source, name, data) {
  for (let i = 0; i < channels.length; ++i) {
    const channel = channels[i];

    if (channel === source) continue; // Don't self-send.
    if (channel[_name] !== name) continue;
    if (channel[_closed]) continue;

    const go = () => {
      if (channel[_closed]) return;
      const event = new MessageEvent("message", {
        data: core.deserialize(data), // TODO(bnoordhuis) Cache immutables.
        origin: "http://127.0.0.1",
      });
      setIsTrusted(event, true);
      setTarget(event, channel);
      channel.dispatchEvent(event);
    };

    defer(go);
  }
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
    webidl.requiredArguments(arguments.length, 1, prefix);

    this[_name] = webidl.converters["DOMString"](name, prefix, "Argument 1");

    this[webidl.brand] = webidl.brand;

    ArrayPrototypePush(channels, this);

    if (rid === null) {
      // Create the rid immediately, otherwise there is a time window (and a
      // race condition) where messages can get lost, because recv() is async.
      rid = ops.op_broadcast_subscribe();
      recv();
    }
  }

  postMessage(message) {
    webidl.assertBranded(this, BroadcastChannelPrototype);

    const prefix = "Failed to execute 'postMessage' on 'BroadcastChannel'";
    webidl.requiredArguments(arguments.length, 1, prefix);

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
    defer(() => {
      if (!this[_closed]) {
        op_broadcast_send(rid, this[_name], data);
      }
    });
  }

  close() {
    webidl.assertBranded(this, BroadcastChannelPrototype);
    this[_closed] = true;

    const index = ArrayPrototypeIndexOf(channels, this);
    if (index === -1) return;

    ArrayPrototypeSplice(channels, index, 1);
    if (channels.length === 0) {
      ops.op_broadcast_unsubscribe(rid);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(BroadcastChannelPrototype, this),
        keys: [
          "name",
          "onmessage",
          "onmessageerror",
        ],
      }),
      inspectOptions,
    );
  }
}

defineEventHandler(BroadcastChannel.prototype, "message");
defineEventHandler(BroadcastChannel.prototype, "messageerror");
const BroadcastChannelPrototype = BroadcastChannel.prototype;

export { BroadcastChannel };
