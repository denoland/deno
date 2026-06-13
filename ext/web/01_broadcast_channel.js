// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference path="../../core/internal.d.ts" />

(function () {
const { core, primordials } = __bootstrap;
const {
  op_broadcast_deserialize,
  op_broadcast_free,
  op_broadcast_recv,
  op_broadcast_send,
  op_broadcast_serialize,
  op_broadcast_subscribe,
  op_broadcast_unsubscribe,
} = core.ops;
const {
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  Uint8Array,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const {
  defineEventHandler,
  EventTarget,
  setIsTrusted,
  setTarget,
} = core.loadExtScript("ext:deno_web/02_event.js");
const { defer } = core.loadExtScript("ext:deno_web/02_timers.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

const _name = Symbol("[[name]]");
const _closed = Symbol("[[closed]]");
const _refed = Symbol("[[refed]]");
const refBroadcastChannel = Symbol("refBroadcastChannel");

const channels = [];
let rid = null;
let recvPromise = null;
let refedBroadcastChannelsCount = 0;

async function recv() {
  while (channels.length > 0) {
    recvPromise = op_broadcast_recv(rid);
    if (refedBroadcastChannelsCount === 0) {
      core.unrefOpPromise(recvPromise);
    }
    const message = await recvPromise;
    recvPromise = null;

    if (message === null) {
      break;
    }

    const { 0: name, 1: data, 2: sabId } = message;
    dispatch(null, name, new Uint8Array(data), sabId);
    // The SharedArrayBuffer backing stores (if any) have been deserialized for
    // every local channel above; release the stash entry.
    if (sabId !== 0) op_broadcast_free(sabId);
  }

  core.close(rid);
  rid = null;
}

function dispatch(source, name, data, sabId) {
  for (let i = 0; i < channels.length; ++i) {
    const channel = channels[i];

    if (channel === source) continue; // Don't self-send.
    if (channel[_name] !== name) continue;
    if (channel[_closed]) continue;

    // Deserialize eagerly (synchronously) while the out-of-band
    // SharedArrayBuffer backing stores referenced by `sabId` are still
    // available; only the event dispatch is deferred. A single broadcast
    // message can be delivered to many receivers, so each gets its own
    // deserialized copy.
    // TODO(bnoordhuis) Cache immutables.
    const messageData = op_broadcast_deserialize(
      data,
      sabId,
      core.getCloneableDeserializers(),
    );

    const go = () => {
      if (channel[_closed]) return;
      const event = new MessageEvent("message", {
        data: messageData,
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
  [_refed] = true;

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
    refedBroadcastChannelsCount++;

    if (rid === null) {
      // Create the rid immediately, otherwise there is a time window (and a
      // race condition) where messages can get lost, because recv() is async.
      rid = op_broadcast_subscribe();
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

    // Serialize the message, carrying any SharedArrayBuffer backing stores
    // out-of-band (referenced by `sabId`) so the message can be deserialized by
    // an arbitrary number of receivers. `sabId` is 0 when there are none.
    const { 0: data, 1: sabId } = op_broadcast_serialize(message, null);

    // Send to other listeners in this VM.
    dispatch(this, this[_name], data, sabId);

    // Send to listeners in other VMs. This must happen before returning from
    // postMessage(), otherwise close() immediately after postMessage() can
    // cancel the deferred send before other workers observe the message.
    op_broadcast_send(rid, this[_name], data, sabId);

    // In-VM dispatch deserialized eagerly and op_broadcast_send moved a clone
    // of the backing stores into the cross-VM message, so the sender's stash
    // entry is no longer needed.
    if (sabId !== 0) op_broadcast_free(sabId);
  }

  [refBroadcastChannel](ref) {
    if (ref && !this[_refed] && !this[_closed]) {
      refedBroadcastChannelsCount++;
      if (refedBroadcastChannelsCount === 1 && recvPromise) {
        core.refOpPromise(recvPromise);
      }
      this[_refed] = true;
    } else if (!ref && this[_refed] && !this[_closed]) {
      refedBroadcastChannelsCount--;
      if (refedBroadcastChannelsCount === 0 && recvPromise) {
        core.unrefOpPromise(recvPromise);
      }
      this[_refed] = false;
    }
  }

  close() {
    webidl.assertBranded(this, BroadcastChannelPrototype);
    this[_closed] = true;

    const index = ArrayPrototypeIndexOf(channels, this);
    if (index === -1) return;

    if (this[_refed]) {
      refedBroadcastChannelsCount--;
      if (refedBroadcastChannelsCount === 0 && recvPromise) {
        core.unrefOpPromise(recvPromise);
      }
      this[_refed] = false;
    }

    ArrayPrototypeSplice(channels, index, 1);
    if (channels.length === 0 && rid !== null) {
      op_broadcast_unsubscribe(rid);
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

return { BroadcastChannel, refBroadcastChannel };
})();
