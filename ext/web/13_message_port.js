// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

(function () {
const { core, primordials } = __bootstrap;
const {
  op_message_port_create_entangled,
  op_message_port_post_message,
  op_message_port_post_message_raw,
  op_message_port_recv_message,
  op_message_port_recv_message_sync,
} = core.ops;
const {
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypePush,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromiseResolve,
  queueMicrotask,
  SafeArrayIterator,
  SafeSet,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
  TypeErrorPrototype,
} = primordials;
const {
  InterruptedPrototype,
  isArrayBuffer,
} = core;
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");

// Lazy-load createFilteredInspectProxy from console to avoid
// circular dependency at load time. Only needed for custom inspect.
let _createFilteredInspectProxy;
function getCreateFilteredInspectProxy() {
  if (!_createFilteredInspectProxy) {
    _createFilteredInspectProxy = core.loadExtScript(
      "ext:deno_web/01_console.js",
    ).createFilteredInspectProxy;
  }
  return _createFilteredInspectProxy;
}

const {
  defineEventHandler,
  EventTarget,
  MessageEvent,
  setEventTargetData,
  setIsTrusted,
} = core.loadExtScript("ext:deno_web/02_event.js");

const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

// counter of how many message ports are actively refed
// either due to the existence of "message" event listeners or
// explicit calls to ref/unref (in the case of node message ports)
let refedMessagePortsCount = 0;

class MessageChannel {
  /** @type {MessagePort} */
  #port1;
  /** @type {MessagePort} */
  #port2;

  constructor() {
    this[webidl.brand] = webidl.brand;
    const { 0: port1Id, 1: port2Id } = opCreateEntangledMessagePort();
    const port1 = createMessagePort(port1Id);
    const port2 = createMessagePort(port2Id);
    this.#port1 = port1;
    this.#port2 = port2;
  }

  get port1() {
    webidl.assertBranded(this, MessageChannelPrototype);
    return this.#port1;
  }

  get port2() {
    webidl.assertBranded(this, MessageChannelPrototype);
    return this.#port2;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      getCreateFilteredInspectProxy()({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(MessageChannelPrototype, this),
        keys: [
          "port1",
          "port2",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(MessageChannel);
const MessageChannelPrototype = MessageChannel.prototype;

const _id = Symbol("id");
const MessagePortIdSymbol = _id;
const MessagePortReceiveMessageOnPortSymbol = Symbol(
  "MessagePortReceiveMessageOnPort",
);
const _enabled = Symbol("enabled");
const _refed = Symbol("refed");
const _messageEventListenerCount = Symbol("messageEventListenerCount");
const nodeWorkerThreadCloseCb = Symbol("nodeWorkerThreadCloseCb");
const nodeWorkerThreadCloseCbInvoked = Symbol("nodeWorkerThreadCloseCbInvoked");
const refMessagePort = Symbol("refMessagePort");
/** It is used by 99_main.js and worker_threads to
 * unref/ref on the global message event handler count. */
const unrefParentPort = Symbol("unrefParentPort");

/**
 * @param {number} id
 * @returns {MessagePort}
 */
function createMessagePort(id) {
  const port = webidl.createBranded(MessagePort);
  port[core.hostObjectBrand] = "MessagePort";
  setEventTargetData(port);
  port[_id] = id;
  port[_enabled] = false;
  port[_messageEventListenerCount] = 0;
  port[_refed] = false;
  return port;
}

function nodeWorkerThreadMaybeInvokeCloseCb(port) {
  if (
    typeof port[nodeWorkerThreadCloseCb] == "function" &&
    !port[nodeWorkerThreadCloseCbInvoked]
  ) {
    port[nodeWorkerThreadCloseCb]();
    port[nodeWorkerThreadCloseCbInvoked] = true;
  }
}

const _isRefed = Symbol("isRefed");
const _dataPromise = Symbol("dataPromise");

/**
 * Deserialize and dispatch a message on a target EventTarget.
 * @returns {boolean} false if dispatch failed with messageerror
 */
function dispatchPortMessageData(target, data) {
  let message, transferables;
  try {
    const v = deserializeJsMessageData(data);
    message = v[0];
    transferables = v[1];
  } catch (err) {
    const event = new MessageEvent("messageerror", { data: err });
    setIsTrusted(event, true);
    target.dispatchEvent(event);
    return false;
  }
  const event = new MessageEvent("message", {
    data: message,
    // Skip the transferables filter for the common no-transferables case.
    // Passing `undefined` lets the MessageEvent constructor take its cheap
    // `ports == null` branch (a single frozen empty array, no iterator
    // validation) instead of allocating a filtered array per message.
    ports: transferables.length === 0 ? undefined : ArrayPrototypeFilter(
      transferables,
      (t) => ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t),
    ),
  });
  setIsTrusted(event, true);
  target.dispatchEvent(event);
  return true;
}

// Internal intermediate class that holds the ref-count override of
// add/removeEventListener. The user-visible `MessagePort` class extends
// this so `Object.getOwnPropertyNames(MessagePort.prototype)` matches
// Node's reduced surface (no add/removeEventListener as own props), while
// the listener-count bookkeeping still happens for any port instance.
class _MessagePortBase extends EventTarget {
  removeEventListener(...args) {
    if (args[0] === "message") {
      if (--this[_messageEventListenerCount] === 0 && this[_refed]) {
        // Use refMessagePort so the underlying recv op promise is also
        // unrefed in lock-step. Otherwise the runtime's
        // hasMessageEventListener() check (which gates worker exit) and
        // the op's ref count can disagree.
        this[refMessagePort](false);
      }
    }
    super.removeEventListener(...new SafeArrayIterator(args));
  }

  addEventListener(...args) {
    if (args[0] === "message") {
      if (++this[_messageEventListenerCount] === 1 && !this[_refed]) {
        this[refMessagePort](true);
      }
    }
    super.addEventListener(...new SafeArrayIterator(args));
  }
}

class MessagePort extends _MessagePortBase {
  /** @type {number | null} */
  [_id] = null;
  /** @type {boolean} */
  [_enabled] = false;
  [_refed] = false;
  /** @type {Promise<any> | undefined} */
  [_dataPromise] = undefined;
  [_messageEventListenerCount] = 0;

  constructor() {
    super();
    ObjectDefineProperty(this, MessagePortReceiveMessageOnPortSymbol, {
      __proto__: null,
      value: false,
      enumerable: false,
    });
    ObjectDefineProperty(this, nodeWorkerThreadCloseCb, {
      __proto__: null,
      value: null,
      enumerable: false,
    });
    ObjectDefineProperty(this, nodeWorkerThreadCloseCbInvoked, {
      __proto__: null,
      value: false,
      enumerable: false,
    });
    webidl.illegalConstructor();
  }

  /**
   * @param {any} message
   * @param {object[] | StructuredSerializeOptions} transferOrOptions
   */
  postMessage(message, transferOrOptions = { __proto__: null }) {
    webidl.assertBranded(this, MessagePortPrototype);
    const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
    webidl.requiredArguments(arguments.length, 1, prefix, ["value"]);
    const portClosed = this[_id] === null;
    // Fast path: no transferables - serialize and send in one shot,
    // bypassing the JsMessageData serde overhead
    if (
      transferOrOptions === undefined ||
      transferOrOptions === null ||
      (arguments.length <= 1)
    ) {
      if (portClosed) return;
      // Honor markAsUncloneable for top-level postMessage values.
      if (isUncloneable(message)) {
        throw new DOMException(
          "Cannot clone object of unsupported type.",
          "DataCloneError",
        );
      }
      op_message_port_post_message_raw(
        this[_id],
        core.serialize(message, undefined, serializeErrorCb),
      );
      return;
    }
    message = webidl.converters.any(message);
    let options;
    if (
      webidl.type(transferOrOptions) === "Object" &&
      transferOrOptions !== undefined &&
      transferOrOptions[SymbolIterator] !== undefined
    ) {
      const transfer = webidl.converters["sequence<object>"](
        transferOrOptions,
        prefix,
        "Argument 2",
      );
      options = { transfer };
    } else {
      options = webidl.converters.StructuredSerializeOptions(
        transferOrOptions,
        prefix,
        "Argument 2",
      );
    }
    // Validate transfer list BEFORE the closed-port early return so calls
    // like `port.postMessage(null, [arrayBuf, alreadyDetachedPort])` raise
    // the same DataCloneError regardless of whether `this` was already
    // detached when the call was made -- matching Node's behavior.
    const { transfer } = options;
    if (ArrayPrototypeIncludes(transfer, this)) {
      throw new DOMException(
        "Transfer list contains source port",
        "DataCloneError",
      );
    }
    // Validate transfer list: each MessagePort must be entangled (not closed),
    // and there must be no duplicates. Matches Node's error wording so the
    // node_compat suite's specific DataCloneError assertions pass.
    if (transfer.length > 0) {
      const seenPorts = new SafeSet();
      const seenBuffers = new SafeSet();
      for (let i = 0; i < transfer.length; i++) {
        const t = transfer[i];
        if (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t)) {
          if (t[_id] === null) {
            throw new DOMException(
              "MessagePort in transfer list is already detached",
              "DataCloneError",
            );
          }
          if (seenPorts.has(t)) {
            throw new DOMException(
              "Transfer list contains duplicate MessagePort",
              "DataCloneError",
            );
          }
          seenPorts.add(t);
        } else if (isArrayBuffer(t)) {
          if (seenBuffers.has(t)) {
            throw new DOMException(
              "Transfer list contains duplicate ArrayBuffer",
              "DataCloneError",
            );
          }
          seenBuffers.add(t);
        }
      }
    }
    if (portClosed) return;
    const data = serializeJsMessageData(message, transfer);
    op_message_port_post_message(this[_id], data);
  }

  start() {
    webidl.assertBranded(this, MessagePortPrototype);
    if (this[_enabled]) return;
    (async () => {
      this[_enabled] = true;
      while (true) {
        if (this[_id] === null) break;
        let data;
        try {
          this[_dataPromise] = op_message_port_recv_message(
            this[_id],
          );
          if (
            typeof this[nodeWorkerThreadCloseCb] === "function" &&
            !this[_refed]
          ) {
            core.unrefOpPromise(this[_dataPromise]);
          }
          data = await this[_dataPromise];
          this[_dataPromise] = undefined;
        } catch (err) {
          if (ObjectPrototypeIsPrototypeOf(InterruptedPrototype, err)) {
            break;
          }
          nodeWorkerThreadMaybeInvokeCloseCb(this);
          throw err;
        }
        if (data === null) {
          nodeWorkerThreadMaybeInvokeCloseCb(this);
          break;
        }
        if (!dispatchPortMessageData(this, data)) return;
        // Yield long enough for any handler-removed-itself +
        // new-handler-attached cycle (used by `events.once`) to register
        // a fresh listener before the next buffered message is
        // dispatched. V8's optimized `await` can resume on a resolved
        // promise in the same microtask checkpoint as the user's
        // dispatch-time `resolve(...)` callback, so explicitly chain
        // through `queueMicrotask` to put the recv-loop continuation
        // strictly behind any user `once()` re-arm in the microtask
        // queue.
        await new Promise((resolve) => queueMicrotask(() => resolve()));
      }
      this[_enabled] = false;
    })();
  }

  [refMessagePort](ref) {
    if (ref) {
      if (!this[_refed]) {
        refedMessagePortsCount++;
        if (
          this[_dataPromise]
        ) {
          core.refOpPromise(this[_dataPromise]);
        }
        this[_refed] = true;
      }
    } else if (!ref) {
      if (this[_refed]) {
        refedMessagePortsCount--;
        if (
          this[_dataPromise]
        ) {
          core.unrefOpPromise(this[_dataPromise]);
        }
        this[_refed] = false;
      }
    }
  }

  // https://nodejs.org/api/worker_threads.html#portref
  ref() {
    webidl.assertBranded(this, MessagePortPrototype);
    this[refMessagePort](true);
  }

  // https://nodejs.org/api/worker_threads.html#portunref
  unref() {
    webidl.assertBranded(this, MessagePortPrototype);
    this[refMessagePort](false);
  }

  // https://nodejs.org/api/worker_threads.html#porthasref
  hasRef() {
    webidl.assertBranded(this, MessagePortPrototype);
    return this[_refed];
  }

  close(cb) {
    webidl.assertBranded(this, MessagePortPrototype);
    // Node's MessagePort.close accepts an optional callback that's added
    // as a one-shot 'close' listener before the underlying handle is torn
    // down. Web MessagePort.close has no `cb` arg, so this is a strict
    // superset.
    if (typeof cb === "function") {
      this.addEventListener("close", function once() {
        this.removeEventListener("close", once);
        cb();
      });
    }
    if (this[_id] !== null) {
      // Drain any already-queued messages synchronously before closing the
      // resource. Node guarantees that messages sent before the close()
      // call get dispatched even if the receiver closes mid-stream
      // (regression test #22762). Without this, messages buffered after
      // the current async recv resolved but before our handler called
      // close() would be silently dropped.
      const portId = this[_id];
      try {
        while (this[_id] === portId) {
          const data = op_message_port_recv_message_sync(portId);
          if (data === null) break;
          if (!dispatchPortMessageData(this, data)) break;
        }
      } catch {
        // recv failed (already canceled / closed); fall through.
      }
      // The dispatch may have closed the port via a user handler that
      // re-entered close(); only tear down the resource if we still own it.
      if (this[_id] === portId) {
        core.close(portId);
        this[_id] = null;
        nodeWorkerThreadMaybeInvokeCloseCb(this);
      }
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    // Surface `active`/`refed` so Node tests that grep the inspect output
    // for those substrings (e.g. test-worker-message-port-transfer-self)
    // see the matching state. Falls back to the filtered inspect proxy
    // (which produces the WHATWG-style `{ onmessage, onmessageerror }`
    // shape) for the actual rendering.
    return inspect(
      getCreateFilteredInspectProxy()({
        object: {
          active: this[_id] !== null,
          refed: this[_refed],
          onmessage: this.onmessage,
          onmessageerror: this.onmessageerror,
        },
        evaluate: ObjectPrototypeIsPrototypeOf(MessagePortPrototype, this),
        keys: [
          "active",
          "refed",
          "onmessage",
          "onmessageerror",
        ],
      }),
      inspectOptions,
    );
  }
}

defineEventHandler(MessagePort.prototype, "message", function (self) {
  if (self[nodeWorkerThreadCloseCb]) {
    (async () => {
      // delay `start()` until he end of this event loop turn, to give `receiveMessageOnPort`
      // a chance to receive a message first. this is primarily to resolve an issue with
      // a pattern used in `npm:piscina` that results in an indefinite hang
      await PromiseResolve();
      self.start();
    })();
  } else {
    self.start();
  }
});
defineEventHandler(MessagePort.prototype, "messageerror");

webidl.configureInterface(MessagePort);
const MessagePortPrototype = MessagePort.prototype;

core.registerTransferableResource("MessagePort", (port) => {
  const id = port[_id];
  port[_id] = null;
  if (id === null) {
    throw new DOMException(
      "Can not transfer disentangled message port",
      "DataCloneError",
    );
  }
  return id;
}, (id) => createMessagePort(id));

/**
 * @returns {[number, number]}
 */
function opCreateEntangledMessagePort() {
  return op_message_port_create_entangled();
}

/**
 * @param {messagePort.MessageData} messageData
 * @returns {[any, object[]]}
 */
const emptyTransferables = ObjectFreeze([]);

// The web-streams transferable resources (ReadableStream/WritableStream/
// TransformStream) are registered as a side effect of evaluating
// ext:deno_web/06_streams.js, which is lazy. A realm (e.g. a worker) that
// receives a transferred stream may not have loaded that module yet, so on a
// miss force-load it (loadExtScript is idempotent) before resolving.
function resolveTransferableResource(type) {
  let resource = core.getTransferableResource(type);
  if (resource === undefined) {
    core.loadExtScript("ext:deno_web/06_streams.js");
    resource = core.getTransferableResource(type);
  }
  return resource;
}

function deserializeJsMessageData(messageData) {
  // Fast path: no transferables (most common case)
  if (messageData.transferables.length === 0) {
    const deserializers = core.getCloneableDeserializers();
    const data = deserializers
      ? core.deserialize(messageData.data, { deserializers })
      : core.deserialize(messageData.data);
    return [data, emptyTransferables];
  }

  /** @type {object[]} */
  const transferables = [];
  const arrayBufferIdsInTransferables = [];
  const transferredArrayBuffers = [];
  let options;

  if (messageData.transferables.length > 0) {
    const hostObjects = [];
    for (let i = 0; i < messageData.transferables.length; ++i) {
      const transferable = messageData.transferables[i];
      switch (transferable.kind) {
        case "resource": {
          const { 0: type, 1: rid } = transferable.data;
          const hostObj = resolveTransferableResource(type).receive(rid);
          ArrayPrototypePush(transferables, hostObj);
          ArrayPrototypePush(hostObjects, hostObj);
          break;
        }
        case "multiResource": {
          const { 0: type, 1: rids } = transferable.data;
          const hostObj = resolveTransferableResource(type).receive(rids);
          ArrayPrototypePush(transferables, hostObj);
          ArrayPrototypePush(hostObjects, hostObj);
          break;
        }
        case "arrayBuffer": {
          ArrayPrototypePush(transferredArrayBuffers, transferable.data);
          const index = ArrayPrototypePush(transferables, null);
          ArrayPrototypePush(arrayBufferIdsInTransferables, index);
          break;
        }
        default:
          throw new TypeError("Unreachable");
      }
    }

    options = {
      hostObjects,
      transferredArrayBuffers,
    };
  }

  const deserializers = core.getCloneableDeserializers();
  if (!options) {
    options = { deserializers };
  } else {
    options.deserializers = deserializers;
  }
  const data = core.deserialize(messageData.data, options);

  for (let i = 0; i < arrayBufferIdsInTransferables.length; ++i) {
    const id = arrayBufferIdsInTransferables[i];
    transferables[id] = transferredArrayBuffers[i];
  }

  return [data, transferables];
}

/**
 * @param {any} data
 * @param {object[]} transferables
 * @returns {messagePort.MessageData}
 */
const emptySerializedTransferables = ObjectFreeze([]);
const serializeErrorCb = (err) => {
  // V8's ValueSerializer reports "Unsupported object type" when the host
  // delegate refuses to serialize an object -- for the workerData case
  // that's specifically a transferable (e.g. MessagePort) that wasn't
  // listed in the transferList. Node's error message is more descriptive
  // and the node_compat suite asserts on it verbatim, so rewrite it here.
  if (err === "Unsupported object type") {
    throw new DOMException(
      "Object that needs transfer was found in message but not listed in transferList",
      "DataCloneError",
    );
  }
  throw new DOMException(err, "DataCloneError");
};

function serializeJsMessageData(data, transferables) {
  const { isDetachedBuffer } = core.loadExtScript("ext:deno_web/06_streams.js");

  // Honor markAsUncloneable at the top level. V8's ValueSerializer
  // can't see the JS-only symbol, so check here before invoking it.
  if (isUncloneable(data) && !ArrayPrototypeIncludes(transferables, data)) {
    throw new DOMException(
      "Cannot clone object of unsupported type.",
      "DataCloneError",
    );
  }

  // Fast path: no transferables (most common case)
  if (transferables.length === 0) {
    const serializedData = core.serialize(data, undefined, serializeErrorCb);
    return {
      data: serializedData,
      transferables: emptySerializedTransferables,
    };
  }

  const hostObjects = [];
  const transferredArrayBuffers = [];
  for (let i = 0, j = 0; i < transferables.length; i++) {
    const t = transferables[i];
    if (isArrayBuffer(t)) {
      if (
        ArrayBufferPrototypeGetByteLength(t) === 0 &&
        isDetachedBuffer(t)
      ) {
        throw new DOMException(
          `ArrayBuffer at index ${j} is already detached`,
          "DataCloneError",
        );
      }
      j++;
      ArrayPrototypePush(transferredArrayBuffers, t);
    } else if (t[core.hostObjectBrand]) {
      ArrayPrototypePush(hostObjects, t);
    }
  }

  const options = {
    hostObjects,
    transferredArrayBuffers,
  };

  const serializedData = core.serialize(data, options, serializeErrorCb);

  /** @type {messagePort.Transferable[]} */
  const serializedTransferables = [];

  let arrayBufferI = 0;
  for (let i = 0; i < transferables.length; ++i) {
    const transferable = transferables[i];
    if (transferable[core.hostObjectBrand]) {
      const type = transferable[core.hostObjectBrand];
      const transferableResource = core.getTransferableResource(type);
      if (!transferableResource) {
        throw new DOMException("Value not transferable", "DataCloneError");
      }
      const rid = transferableResource.send(transferable);
      if (typeof rid === "number") {
        ArrayPrototypePush(serializedTransferables, {
          kind: "resource",
          data: [type, rid],
        });
      } else {
        ArrayPrototypePush(serializedTransferables, {
          kind: "multiResource",
          data: [type, rid],
        });
      }
    } else if (isArrayBuffer(transferable)) {
      ArrayPrototypePush(serializedTransferables, {
        kind: "arrayBuffer",
        data: transferredArrayBuffers[arrayBufferI],
      });
      arrayBufferI++;
    } else {
      throw new DOMException("Value not transferable", "DataCloneError");
    }
  }

  return {
    data: serializedData,
    transferables: serializedTransferables,
  };
}

webidl.converters.StructuredSerializeOptions = webidl
  .createDictionaryConverter(
    "StructuredSerializeOptions",
    [
      {
        key: "transfer",
        converter: webidl.converters["sequence<object>"],
        get defaultValue() {
          return [];
        },
      },
    ],
  );

// Marker symbol for Web API types whose specs explicitly mark them as
// non-serializable. V8's structured clone serialiser doesn't know about Web
// API "platform" types (they're plain JS objects from V8's perspective with
// no enumerable own properties), so without this opt-out the fast
// `core.structuredClone` path silently round-trips them as `{}`, matching
// neither the Web Platform spec nor Node's behaviour, which both raise
// `DataCloneError`.
//
// Each non-serializable class installs this symbol on its prototype via
// `markNotSerializable()`. The descriptor is non-enumerable and
// non-configurable so it can't be hidden, deleted, or overridden on the
// instance.
const kNotSerializable = Symbol("[[NotSerializable]]");

function markNotSerializable(target) {
  ObjectDefineProperty(target, kNotSerializable, {
    __proto__: null,
    value: true,
    enumerable: false,
    writable: false,
    configurable: false,
  });
}

// Per-instance "uncloneable" marker used by `worker_threads.markAsUncloneable`.
// Unlike `kNotSerializable`, this is settable on individual objects (not just
// prototypes) and must be ignorable for ArrayBuffer (Node spec). When set on
// the *value* at the top of postMessage / structuredClone, the call throws a
// DataCloneError without invoking V8's serializer.
const kUncloneable = Symbol("[[Uncloneable]]");

function markAsUncloneable(target) {
  // Per Node spec: silently no-ops on ArrayBuffer (use markAsUntransferable
  // for those) and on non-object/non-function values.
  if (
    target === null ||
    (typeof target !== "object" && typeof target !== "function")
  ) {
    return;
  }
  if (isArrayBuffer(target)) return;
  ObjectDefineProperty(target, kUncloneable, {
    __proto__: null,
    value: true,
    enumerable: false,
    writable: false,
    configurable: false,
  });
}

function isUncloneable(value) {
  if (value === null) return false;
  const t = typeof value;
  if (t !== "object" && t !== "function") return false;
  // Skip the check if the value is itself a constructor's prototype
  // object (e.g. `MockResponse.prototype` in Node's mark-as-uncloneable
  // test). The marker is intended to flag instances of a marked class,
  // not unrelated prototype objects further down the chain, and Node's
  // V8 serializer special-cases this via host-object brand checks. We
  // approximate the brand check by treating `value.constructor.prototype
  // === value` as "this is a prototype object, allow cloning unless the
  // marker is set on the value itself".
  if (
    value[kUncloneable] === true &&
    !isOwnPrototypeObject(value, kUncloneable)
  ) {
    return true;
  }
  return ObjectHasOwn(value, kUncloneable) === true;
}

function isOwnPrototypeObject(value, sym) {
  if (ObjectHasOwn(value, sym)) return false; // marker set on value itself
  try {
    return value.constructor?.prototype === value;
  } catch {
    return false;
  }
}

// Streams self-register their prototypes at the bottom of 06_streams.js.
// Fetch types (Headers / Request / Response) call `markNotSerializable`
// themselves at the bottom of their respective modules.

function structuredClone(value, options) {
  // Fast path for primitives that StructuredSerialize returns by reference:
  // null, undefined, boolean, number, string, bigint. These don't need the
  // StructuredSerializeOptions dictionary conversion, the not-serializable
  // marker check, or the V8 ValueSerializer/Deserializer round-trip.
  // Symbol falls through to the slow path which throws DataCloneError;
  // 0-arg calls also fall through so requiredArguments can throw. We also
  // require `options === undefined` so the slow-path StructuredSerializeOptions
  // converter still rejects malformed second arguments
  // (e.g. `structuredClone(42, "not-an-object")` keeps throwing TypeError).
  if (arguments.length >= 1 && options === undefined) {
    if (value === null) return value;
    const t = typeof value;
    if (t !== "object" && t !== "function" && t !== "symbol") {
      return value;
    }
  }

  const prefix = "Failed to execute 'structuredClone'";
  webidl.requiredArguments(arguments.length, 1, prefix, ["value"]);
  options = webidl.converters.StructuredSerializeOptions(
    options,
    prefix,
    "Argument 2",
  );

  // NOTE: This only catches non-serializable types at the top level.
  // Nested non-serializable objects (e.g. { x: new Response() }) will
  // still silently serialize as {} because V8's ValueSerializer doesn't
  // know about Web API platform types. Fixing this fully requires a
  // custom V8 serializer delegate in C++/Rust.
  // Skip the check when the value itself is in the transfer list, since
  // transferring is not the same as serializing.
  if (
    value !== null && typeof value === "object" &&
    !ArrayPrototypeIncludes(options.transfer, value)
  ) {
    // Same prototype-object exemption as in isUncloneable: a marker on a
    // class's prototype means "instances are uncloneable", not "the
    // prototype object itself is".
    const blocked = (value[kNotSerializable] === true &&
      !isOwnPrototypeObject(value, kNotSerializable)) ||
      isUncloneable(value);
    if (blocked) {
      throw new DOMException(
        "Cannot clone object of unsupported type.",
        "DataCloneError",
      );
    }
  }

  // Fast-path, avoiding round-trip serialization and deserialization
  if (options.transfer.length === 0) {
    try {
      return core.structuredClone(value);
    } catch (e) {
      if (ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e)) {
        throw new DOMException(e.message, "DataCloneError");
      }
      throw e;
    }
  }

  const messageData = serializeJsMessageData(value, options.transfer);
  return deserializeJsMessageData(messageData)[0];
}

return {
  deserializeJsMessageData,
  markAsUncloneable,
  markNotSerializable,
  MessageChannel,
  MessagePort,
  MessagePortIdSymbol,
  MessagePortPrototype,
  MessagePortReceiveMessageOnPortSymbol,
  nodeWorkerThreadCloseCb,
  nodeWorkerThreadCloseCbInvoked,
  // `refedMessagePortsCount` is a mutable module-level counter. Before
  // ext/web was converted to lazy-loaded IIFE scripts (#33760), this module
  // used real ESM `export`s, so consumers (runtime/js/99_main.js'
  // `hasMessageEventListener()`) observed a *live binding* that tracked the
  // counter. A plain `refedMessagePortsCount` property here would instead
  // capture a one-time snapshot of `0`, silently breaking the worker
  // idle-termination check for refed Node message ports (#23169). Expose it
  // as a getter to restore the live-binding behavior.
  get refedMessagePortsCount() {
    return refedMessagePortsCount;
  },
  refMessagePort,
  serializeJsMessageData,
  structuredClone,
  unrefParentPort,
};
})();
