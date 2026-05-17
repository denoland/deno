// Copyright 2018-2026 the Deno authors. MIT license.

// Node-flavoured MessagePort / MessageChannel for `node:worker_threads`.
//
// Sits on top of the message-port transport in `ext/web/message_port.rs`
// (entangled MPSC channels, transferable resource handling). The JS class
// here replaces the Web `MessagePort` wrapper for Node-side usage:
//
//   * `MessagePort extends EventEmitter` -- arbitrary event names work,
//     `.on`/`.once`/`.off`/`.emit`/`.removeAllListeners`/`.eventNames`/
//     `.listenerCount` are inherited.
//   * `addEventListener` / `removeEventListener` are also exposed for
//     Web-compat: handlers receive a `MessageEvent` (data) or `Event`
//     (close) like in the browser. Both surfaces coexist on the same
//     port and both fire when a message arrives.
//   * Two transport flavours: regular ports created via `MessageChannel`
//     are backed by a `MessagePortResource` rid and call into the
//     `op_message_port_*` ops; the worker-side `parentPort` is backed
//     by the per-thread worker channel and calls into the
//     `op_worker_*_message*` ops, riding on top of the message loop in
//     `runtime/js/99_main.js`.
//
// The web `MessagePort` registration for the transferable kind
// "MessagePort" is overridden here so that ports transferred Node->Node
// arrive as Node MessagePort instances.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  op_message_port_create_entangled,
  op_message_port_post_message,
  op_message_port_post_message_raw,
  op_message_port_recv_message,
  op_message_port_recv_message_sync,
  op_worker_post_message,
  op_worker_post_message_raw,
  op_mark_as_untransferable,
} = core.ops;

const {
  ArrayIsArray,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  FunctionPrototypeCall,
  MapPrototypeForEach,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  PromiseResolve,
  ReflectApply,
  SafeMap,
  SafeSet,
  SafeWeakSet,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypeError,
} = primordials;
const { isArrayBuffer } = core;
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  Event,
  MessageEvent,
  setIsTrusted,
  setTarget,
} = core.loadExtScript("ext:deno_web/02_event.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const {
  deserializeJsMessageData,
  serializeJsMessageData,
  kNotSerializable,
  MessagePortIdSymbol,
  MessagePortPrototype: WebMessagePortPrototype,
  unrefParentPort,
} = core.loadExtScript("ext:deno_web/13_message_port.js");
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");

// --- internal slots ---------------------------------------------------

// rid for the underlying MessagePortResource (null for parentPort and
// after the port has been transferred / closed).
const kPortId = MessagePortIdSymbol;
// "port"   -> backed by op_message_port_* (rid kept in kPortId)
// "parent" -> backed by op_worker_* (single per-thread channel)
const kTransportKind = Symbol("MessagePort.transport");
// receive loop running?
const kStarted = Symbol("MessagePort.started");
// in-flight recv promise for ref/unref
const kRecvPromise = Symbol("MessagePort.recvPromise");
// is the port currently refed?
const kRefed = Symbol("MessagePort.refed");
// listeners registered via addEventListener (key: event name, val: Map<orig, wrapper>)
const kEventListeners = Symbol("MessagePort.eventListeners");
// has the port been closed locally?
const kClosed = Symbol("MessagePort.closed");
// onmessage / onmessageerror property handlers
const kOnMessage = Symbol("MessagePort.onmessage");
const kOnMessageError = Symbol("MessagePort.onmessageerror");
// onmessage handler is registered via the public accessor
const kHasOnMessageProperty = Symbol("MessagePort.hasOnMessageProperty");
// Messages received between listener-removal and listener-readd. The
// underlying recv op can't be cancelled mid-flight, so if data arrives
// while the consumer set is empty, we buffer it here and drain on the
// next `newListener` event.
const kPendingMessages = Symbol("MessagePort.pendingMessages");
// Transient slot used to pass `MessageEvent.ports` from fireMessage to
// the addEventListener dispatch inside _emitWithEvent.
const kIncomingPorts = Symbol("MessagePort.incomingPorts");

// --- mark-as-untransferable / mark-as-uncloneable --------------------

const kNodeUntransferable = SymbolFor(
  "nodejs.worker_threads.untransferable",
);
const kNodeUncloneable = SymbolFor("nodejs.worker_threads.uncloneable");

// V8 ArrayBuffers don't accept symbol-keyed own properties cheaply, so
// we track marked buffers in a WeakSet. Host objects (e.g. MessagePort)
// get a symbol marker; both are checked at postMessage time.
const markedUntransferableBuffers = new SafeWeakSet();

function markAsUntransferable(value: unknown) {
  if (value === null) return;
  const t = typeof value;
  if (t !== "object" && t !== "function") return;
  if (isArrayBuffer(value)) {
    op_mark_as_untransferable(value as ArrayBuffer);
    markedUntransferableBuffers.add(value);
    return;
  }
  // deno-lint-ignore no-explicit-any
  (value as any)[kNodeUntransferable] = true;
}

function isMarkedAsUntransferable(value: unknown): boolean {
  if (value === null) return false;
  const t = typeof value;
  if (t !== "object" && t !== "function") return false;
  if (isArrayBuffer(value)) {
    return markedUntransferableBuffers.has(value);
  }
  // Node only treats own-properties as marked, so the mark doesn't
  // propagate down a prototype chain.
  return ObjectHasOwn(value, kNodeUntransferable) &&
    // deno-lint-ignore no-explicit-any
    (value as any)[kNodeUntransferable] === true;
}

function markAsUncloneable(value: unknown) {
  if (value === null) return;
  const t = typeof value;
  if (t !== "object" && t !== "function") return;
  // deno-lint-ignore no-explicit-any
  (value as any)[kNodeUncloneable] = true;
}

// --- MessagePort -----------------------------------------------------

// EventTarget-style facade kept on a base class so that
// `Object.getOwnPropertyNames(MessagePort.prototype)` matches Node and
// doesn't enumerate addEventListener/removeEventListener/dispatchEvent.
class _MessagePortBase extends EventEmitter {
  addEventListener(name: string, listener: unknown, _options?: unknown) {
    if (typeof listener !== "function" && !isEventHandlerObject(listener)) {
      return;
    }
    let map = this[kEventListeners];
    if (!map) {
      map = new SafeMap();
      this[kEventListeners] = map;
    }
    let perEvent = map.get(name);
    if (!perEvent) {
      perEvent = new SafeMap();
      map.set(name, perEvent);
    }
    if (perEvent.has(listener)) return;
    perEvent.set(listener, true);
    if (name === "message") {
      autoRefOnFirstMessageListener(this as unknown as MessagePort);
      (this as unknown as MessagePort).start();
    }
  }

  removeEventListener(name: string, listener: unknown) {
    const map = this[kEventListeners];
    if (!map) return;
    const perEvent = map.get(name);
    if (!perEvent) return;
    perEvent.delete(listener);
    if (name === "message") autoUnrefIfIdle(this as unknown as MessagePort);
  }

  dispatchEvent(event: Event) {
    // dispatchEvent routes through emit so that EE listeners receive the
    // unwrapped value (matching Node, where the two listener sets are
    // shared) -- but pass the event object itself so addEventListener
    // handlers see what the caller provided rather than a synthesised one.
    return _emitWithEvent(
      this as unknown as MessagePort,
      event.type,
      event,
      event,
    );
  }

  // Unified emit: fires EE listeners AND addEventListener-style listeners
  // AND the onmessage/onmessageerror property handlers (when relevant).
  // Node's MessagePort treats these as one listener set, so a single
  // `port.emit('foo', 'bar')` call must reach every consumer.
  emit(name: string, ...args: unknown[]): boolean {
    const port = this as unknown as MessagePort;
    if (name === "message" || name === "messageerror") {
      const data = args[0];
      const ports = port[kIncomingPorts] as MessagePort[] | undefined;
      const event = new MessageEvent(name, {
        data,
        ports: ports ?? [],
      });
      setIsTrusted(event, true);
      setTarget(event, port);
      return _emitWithEvent(port, name, args, event);
    }
    if (name === "close") {
      const event = new Event("close");
      setIsTrusted(event, true);
      setTarget(event, port);
      return _emitWithEvent(port, name, args, event);
    }
    // Arbitrary event name: addEventListener handlers get a generic event
    // with `detail` set to the first arg (matches Node's CustomEvent shape).
    let event: Event | undefined;
    const aelMap = port[kEventListeners]?.get(name);
    if (aelMap && aelMap.size > 0) {
      event = new Event(name);
      // deno-lint-ignore no-explicit-any
      (event as any).detail = args[0];
      setIsTrusted(event, true);
      setTarget(event, port);
    }
    return _emitWithEvent(port, name, args, event);
  }
}

function _emitWithEvent(
  port: MessagePort,
  name: string,
  args: unknown[] | unknown,
  event: Event | undefined,
): boolean {
  // EE side -- bare args.
  const eeArgsArr = ArrayIsArray(args) ? args as unknown[] : [args];
  const eeCallArgs: unknown[] = [name];
  for (let i = 0; i < eeArgsArr.length; i++) {
    ArrayPrototypePush(eeCallArgs, eeArgsArr[i]);
  }
  // deno-lint-ignore no-explicit-any
  const eeEmit = (EventEmitter.prototype as any).emit as (
    ...args: unknown[]
  ) => boolean;
  const hadEE = ReflectApply(eeEmit, port, eeCallArgs);
  // Property handlers + addEventListener handlers -- event object.
  if (event !== undefined) {
    if (name === "message" && port[kOnMessage]) {
      invokeEventHandlerListener(port[kOnMessage], event);
    } else if (name === "messageerror" && port[kOnMessageError]) {
      invokeEventHandlerListener(port[kOnMessageError], event);
    }
    const listeners = port[kEventListeners]?.get(name);
    if (listeners) {
      MapPrototypeForEach(listeners, (_v, orig) => {
        invokeEventHandlerListener(orig, event);
      });
    }
  }
  if (name === "message") autoUnrefIfIdle(port);
  return hadEE;
}

// Internal class. The public `MessagePort` (exported below) is a
// function wrapper that throws on user-side construction. The factories
// use `webidl.createBranded` which bypasses the constructor entirely,
// so the body here is never executed in practice.
class MessagePort extends _MessagePortBase {
  // --- onmessage / onmessageerror property accessors -----------------

  get onmessage() {
    return this[kOnMessage] ?? null;
  }
  set onmessage(handler) {
    const had = !!this[kOnMessage];
    this[kOnMessage] = typeof handler === "function" ? handler : null;
    if (this[kOnMessage] && !had) {
      this[kHasOnMessageProperty] = true;
      autoRefOnFirstMessageListener(this);
      this.start();
    } else if (!this[kOnMessage]) {
      this[kHasOnMessageProperty] = false;
    }
  }

  get onmessageerror() {
    return this[kOnMessageError] ?? null;
  }
  set onmessageerror(handler) {
    this[kOnMessageError] = typeof handler === "function" ? handler : null;
  }

  // --- core surface --------------------------------------------------

  postMessage(message: unknown, transferOrOptions: unknown = undefined) {
    const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    const kind = this[kTransportKind];

    // Resolve the transfer list following Node's `postMessage` rules.
    // The second argument may be omitted, null, undefined, an array, any
    // iterable, or an options object with a `.transfer` property that is
    // omitted/null/undefined/array/iterable.
    let transfer: unknown[] | null = null;
    if (
      arguments.length > 1 &&
      transferOrOptions !== undefined &&
      transferOrOptions !== null
    ) {
      transfer = resolveTransferArg(transferOrOptions);
    }

    // Validate the transfer list FIRST -- a closed source port still
    // throws DataCloneError if its transfer list contains an invalid
    // entry (detached MessagePort, duplicate buffer, source self, etc.).
    // Only after validation passes do we silently drop the message on
    // a closed/detached source.
    if (transfer !== null && transfer.length > 0) {
      validateTransferList(transfer, this);
    }

    // `markAsUncloneable` opts out a value from structured-clone (Node
    // ignores it for ArrayBuffers; only host objects are gated).
    // `kNotSerializable` is the web-side marker that Node-private
    // types (FileHandle, Response, ...) install on their prototypes;
    // also reject here so callers see DataCloneError instead of a
    // silent `{}` round-trip. Skip the check when the value is itself
    // in the transfer list (transferring is not the same as cloning).
    const transferred = transfer !== null &&
      transfer.length > 0 &&
      ArrayIsArray(transfer) &&
      ArrayPrototypeIndexOf(transfer, message) !== -1;
    if (
      !transferred &&
      message !== null && typeof message === "object" &&
      !isArrayBuffer(message) &&
      ((message as { [k: symbol]: unknown })[kNodeUncloneable] === true ||
        (message as { [k: symbol]: unknown })[kNotSerializable] === true)
    ) {
      throw new DOMException(
        "Cannot clone object of unsupported type.",
        "DataCloneError",
      );
    }

    if (kind === "port" && this[kPortId] === null) return;
    if (this[kClosed]) return;

    // Fast path: no transferables (most common case).
    if (transfer === null || transfer.length === 0) {
      const buf = core.serialize(message, undefined, dataCloneErrorCb);
      if (kind === "port") {
        op_message_port_post_message_raw(this[kPortId]!, buf);
      } else {
        op_worker_post_message_raw(buf);
      }
      return;
    }

    const data = serializeJsMessageData(message, transfer);
    if (kind === "port") {
      op_message_port_post_message(this[kPortId]!, data);
    } else {
      op_worker_post_message(data);
    }
  }

  start() {
    // For "parent" transport, the message loop lives in 99_main.js and
    // emits MessageEvents on globalThis; the bridge install/teardown is
    // driven by listener add/remove hooks elsewhere.
    if (this[kTransportKind] === "port") {
      startPortRecvLoop(this);
    }
  }

  close(cb?: unknown) {
    if (typeof cb === "function") {
      // Node's close(cb) registers a one-shot 'close' listener.
      // deno-lint-ignore no-explicit-any
      this.once("close", cb as (...args: any[]) => void);
    }
    if (this[kClosed]) return;
    this[kClosed] = true;
    const kind = this[kTransportKind];
    if (kind === "port") {
      const rid = this[kPortId];
      if (rid !== null) {
        // Drain queued messages before tearing down the rid so that any
        // messages already in flight at the moment of close still reach
        // the listener (matches Node and is what
        // test-worker-message-port-close-while-receiving asserts).
        try {
          while (true) {
            const data = op_message_port_recv_message_sync(rid);
            if (data === null) break;
            dispatchIncoming(this, data);
          }
        } catch {
          // Resource already torn down -- nothing more to drain.
        }
        core.close(rid);
        this[kPortId] = null;
      }
    }
    // Fire 'close' on a fresh microtask -- Node emits it asynchronously
    // so any synchronous code that ran in the same turn (other ports
    // closing, transfer-list checks against an already-detached peer)
    // observes the detached state before the listener fires.
    deferFireClose(this);
  }

  ref() {
    setRefed(this, true);
  }

  unref() {
    setRefed(this, false);
  }

  hasRef(): boolean {
    return !!this[kRefed];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect({
      __proto__: { constructor: MessagePort },
      active: !this[kClosed] && (this[kTransportKind] === "parent" ||
        this[kPortId] !== null),
      refed: !!this[kRefed],
    }, inspectOptions);
  }
}

const MessagePortPrototype = MessagePort.prototype;

function isEventHandlerObject(v: unknown): boolean {
  return typeof v === "object" && v !== null &&
    // deno-lint-ignore no-explicit-any
    typeof (v as any).handleEvent === "function";
}

function invokeEventHandlerListener(listener, event: Event) {
  if (typeof listener === "function") {
    try {
      listener(event);
    } catch (err) {
      // Surface to globalThis like web EventTarget does.
      reportError(err);
    }
  } else if (listener && typeof listener.handleEvent === "function") {
    try {
      listener.handleEvent(event);
    } catch (err) {
      reportError(err);
    }
  }
}

function reportError(err: unknown) {
  // Best-effort: defer to core's report-exception so the error event
  // path fires.
  try {
    core.reportException(err);
  } catch {
    // ignore
  }
}

// --- transport-internal -------------------------------------------------

function autoRefOnFirstMessageListener(port: MessagePort) {
  if (port[kTransportKind] === "parent") {
    ensureParentPortBridge(port);
    return;
  }
  if (port[kRefed]) return;
  setRefed(port, true);
}

// Total number of consumers that should keep the port refed: EE
// listeners + addEventListener listeners + onmessage accessor.
function messageConsumerCount(port: MessagePort): number {
  let n = port.listenerCount("message");
  const ael = port[kEventListeners]?.get("message");
  if (ael) n += ael.size;
  if (port[kOnMessage]) n += 1;
  return n;
}

function autoUnrefIfIdle(port: MessagePort) {
  if (messageConsumerCount(port) > 0) return;
  if (port[kTransportKind] === "parent") {
    removeParentPortBridge(port);
    return;
  }
  if (!port[kRefed]) return;
  setRefed(port, false);
}

function setRefed(port: MessagePort, ref: boolean) {
  if (port[kRefed] === ref) return;
  port[kRefed] = ref;
  const kind = port[kTransportKind];
  if (kind === "parent") {
    // Mirror onto globalThis for hasMessageEventListener() in 99_main.js
    // (which treats an unref'd parentPort like having no listeners).
    globalThis[unrefParentPort] = !ref;
    // deno-lint-ignore no-explicit-any
    (port as any)[unrefParentPort] = !ref;
    return;
  }
  const promise = port[kRecvPromise];
  if (promise) {
    if (ref) core.refOpPromise(promise);
    else core.unrefOpPromise(promise);
  }
}

function startPortRecvLoop(port: MessagePort) {
  if (port[kStarted]) return;
  if (port[kTransportKind] !== "port") return;
  if (port[kClosed] || port[kPortId] === null) return;
  port[kStarted] = true;
  (async () => {
    while (!port[kClosed] && port[kPortId] !== null) {
      let data;
      try {
        const p = op_message_port_recv_message(port[kPortId]!);
        port[kRecvPromise] = p;
        if (!port[kRefed]) core.unrefOpPromise(p);
        data = await p;
        port[kRecvPromise] = undefined;
      } catch {
        break;
      }
      if (data === null) {
        // Peer disentangled.
        port[kClosed] = true;
        fireClose(port);
        break;
      }
      dispatchIncoming(port, data);
    }
    port[kStarted] = false;
  })();
}

type PendingEntry =
  | { ok: true; data: unknown; ports: MessagePort[] }
  | { ok: false; err: unknown };

function dispatchIncoming(port: MessagePort, data) {
  if (messageConsumerCount(port) === 0) {
    let buf = port[kPendingMessages] as PendingEntry[] | undefined;
    if (!buf) {
      buf = [];
      port[kPendingMessages] = buf;
    }
    try {
      const v = deserializeJsMessageData(data);
      ArrayPrototypePush(buf, {
        ok: true,
        data: v[0],
        ports: extractPorts(v[1]),
      });
    } catch (err) {
      ArrayPrototypePush(buf, { ok: false, err });
    }
    return;
  }
  let message: unknown;
  let ports: MessagePort[];
  try {
    const v = deserializeJsMessageData(data);
    message = v[0];
    ports = extractPorts(v[1]);
  } catch (err) {
    fireMessageError(port, err);
    return;
  }
  fireMessage(port, message, ports);
}

function extractPorts(transferables: unknown[]): MessagePort[] {
  const out: MessagePort[] = [];
  for (let i = 0; i < transferables.length; i++) {
    const t = transferables[i];
    if (
      t !== null && typeof t === "object" &&
      (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t) ||
        ObjectPrototypeIsPrototypeOf(WebMessagePortPrototype, t))
    ) {
      ArrayPrototypePush(out, t as MessagePort);
    }
  }
  return out;
}

function drainPendingMessages(port: MessagePort) {
  const buf = port[kPendingMessages] as PendingEntry[] | undefined;
  if (!buf || buf.length === 0) return;
  port[kPendingMessages] = undefined;
  for (let i = 0; i < buf.length; i++) {
    if (messageConsumerCount(port) === 0) {
      port[kPendingMessages] = ArrayPrototypeSlice(buf, i);
      return;
    }
    const entry = buf[i];
    if (entry.ok) fireMessage(port, entry.data, entry.ports);
    else fireMessageError(port, entry.err);
  }
}

function fireMessage(port: MessagePort, data: unknown, ports?: MessagePort[]) {
  if (ports && ports.length > 0) {
    // Carry transferred ports through emit so _emitWithEvent can attach
    // them to the synthesised MessageEvent for addEventListener listeners.
    port[kIncomingPorts] = ports;
    try {
      port.emit("message", data);
    } finally {
      port[kIncomingPorts] = undefined;
    }
  } else {
    port.emit("message", data);
  }
}

function fireMessageError(port: MessagePort, err: unknown) {
  port.emit("messageerror", err);
}

function fireClose(port: MessagePort) {
  port.emit("close");
}

function deferFireClose(port: MessagePort) {
  PromisePrototypeThen(PromiseResolve(undefined), () => fireClose(port));
}

const dataCloneErrorCb = (err) => {
  throw new DOMException(err, "DataCloneError");
};

// `postMessage` accepts:
//   * Array (used as-is)
//   * iterable (Symbol.iterator returning a proper { next: function } iterator)
//   * options object with `.transfer` of the above (or null/undefined)
// Returns the materialised array, or null when no transfer is supplied.
// Throws Node-shaped TypeError (code: ERR_INVALID_ARG_TYPE) on bad input.
function resolveTransferArg(arg: unknown): unknown[] | null {
  if (ArrayIsArray(arg)) return arg as unknown[];
  if (isProperIterable(arg)) return drainIterable(arg as object);
  if (typeof arg === "object" && arg !== null) {
    // Options-object form. Check it isn't iterable (handled above).
    const t = (arg as { transfer?: unknown }).transfer;
    // Only `undefined` opts out; anything else (including `null`) must
    // be a real iterable to match Node's stricter options validation.
    if (t === undefined) return null;
    if (ArrayIsArray(t)) return t as unknown[];
    if (isProperIterable(t)) return drainIterable(t as object);
    throw makeInvalidArgType(
      "Optional options.transfer argument must be an iterable",
    );
  }
  throw makeInvalidArgType(
    "Optional transferList argument must be an iterable",
  );
}

function drainIterable(v: object): unknown[] {
  const out: unknown[] = [];
  // deno-lint-ignore no-explicit-any
  const iter = (v as any)[SymbolIterator]();
  // deno-lint-ignore no-explicit-any
  const nextFn = iter.next as () => any;
  while (true) {
    const step = FunctionPrototypeCall(nextFn, iter);
    if (step.done) break;
    ArrayPrototypePush(out, step.value);
  }
  return out;
}

// An iterable here means: object/function with a `Symbol.iterator` that
// returns an iterator whose `.next` is callable. Tests exercise the
// shape-validation explicitly (e.g. `{ [Symbol.iterator]() { return {} } }`
// must fail, not silently iterate).
function isProperIterable(v: unknown): boolean {
  if (v === null) return false;
  const t = typeof v;
  if (t !== "object" && t !== "function") return false;
  const iterFn = (v as { [k: symbol]: unknown })[SymbolIterator];
  if (typeof iterFn !== "function") return false;
  const iter = FunctionPrototypeCall(iterFn as () => unknown, v);
  if (iter === null || typeof iter !== "object") return false;
  return typeof (iter as { next?: unknown }).next === "function";
}

function makeInvalidArgType(message: string): TypeError {
  const err = new TypeError(message);
  // deno-lint-ignore no-explicit-any
  (err as any).code = "ERR_INVALID_ARG_TYPE";
  return err;
}

function validateTransferList(transfer: unknown[], sourcePort: MessagePort) {
  const seenPorts = new SafeSet();
  const seenBuffers = new SafeSet();
  for (let i = 0; i < transfer.length; i++) {
    const item = transfer[i];
    if (item === sourcePort) {
      throw new DOMException(
        "Transfer list contains source port",
        "DataCloneError",
      );
    }
    if (isMessagePortLike(item)) {
      if (seenPorts.has(item)) {
        throw new DOMException(
          "Transfer list contains duplicate MessagePort",
          "DataCloneError",
        );
      }
      seenPorts.add(item);
      if ((item as { [k: symbol]: unknown })[kPortId] === null) {
        throw new DOMException(
          "MessagePort in transfer list is already detached",
          "DataCloneError",
        );
      }
    } else if (isArrayBuffer(item)) {
      if (seenBuffers.has(item)) {
        throw new DOMException(
          "Transfer list contains duplicate ArrayBuffer",
          "DataCloneError",
        );
      }
      seenBuffers.add(item);
      if (markedUntransferableBuffers.has(item)) {
        throw new DOMException(
          "Value not transferable",
          "DataCloneError",
        );
      }
    }
    if (
      item !== null && typeof item === "object" &&
      (item as { [k: symbol]: unknown })[kNodeUntransferable] === true
    ) {
      throw new DOMException("Value not transferable", "DataCloneError");
    }
  }
}

function isMessagePortLike(v: unknown): boolean {
  if (v === null || typeof v !== "object") return false;
  return ObjectPrototypeIsPrototypeOf(MessagePortPrototype, v) ||
    ObjectPrototypeIsPrototypeOf(WebMessagePortPrototype, v);
}

// --- factories ----------------------------------------------------------

// Hook into EE's meta-events so that `.on/.once/.addListener/.prependListener`
// for 'message' (and 'messageerror') all auto-ref the port without needing
// per-method overrides on the MessagePort class itself -- those overrides
// would leak as own properties on MessagePort.prototype and break Node's
// `Object.getOwnPropertyNames(MessagePort.prototype)` shape.
// deno-lint-ignore no-explicit-any
const eeOn = (EventEmitter.prototype as any).on as (
  this: unknown,
  name: string,
  listener: (...args: unknown[]) => void,
) => unknown;

function installListenerHooks(port: MessagePort) {
  FunctionPrototypeCall(eeOn, port, "newListener", (name: string) => {
    if (name === "message" || name === "messageerror") {
      autoRefOnFirstMessageListener(port);
      if (name === "message") {
        port.start();
        // Deliver any messages that arrived while we had no consumer.
        // Deferred to a microtask so the user's `.on()` call returns
        // before listeners fire (matches Node, where the first message
        // never delivers synchronously inside the same .on() call).
        PromisePrototypeThen(
          PromiseResolve(undefined),
          () => drainPendingMessages(port),
        );
      }
    } else if (name === "close") {
      // A 'close' listener also needs the recv loop running so that the
      // peer-disentangle signal (recv returns null) can be observed and
      // fire the event. Don't ref the loop -- a sole 'close' listener
      // should not keep the worker alive.
      port.start();
    }
  });
  FunctionPrototypeCall(eeOn, port, "removeListener", (name: string) => {
    if (name === "message") autoUnrefIfIdle(port);
  });
}

function createPortFromRid(rid: number): MessagePort {
  const port = webidl.createBranded(MessagePort);
  // EE init (sets _events, _eventsCount, _maxListeners).
  FunctionPrototypeCall(EventEmitter.init, port);
  port[core.hostObjectBrand] = "MessagePort";
  port[kTransportKind] = "port";
  port[kPortId] = rid;
  port[kStarted] = false;
  port[kRefed] = false;
  port[kClosed] = false;
  port[kEventListeners] = undefined;
  port[kOnMessage] = null;
  port[kOnMessageError] = null;
  installListenerHooks(port);
  return port;
}

function createParentPort(): MessagePort {
  const port = webidl.createBranded(MessagePort);
  FunctionPrototypeCall(EventEmitter.init, port);
  // No hostObjectBrand: parentPort is not transferable.
  port[kTransportKind] = "parent";
  port[kPortId] = null;
  port[kStarted] = true; // parent transport is implicitly running
  port[kRefed] = false; // start unrefed; bridge installs on first listener
  port[kClosed] = false;
  port[kEventListeners] = undefined;
  port[kOnMessage] = null;
  port[kOnMessageError] = null;
  installListenerHooks(port);
  return port;
}

// Bridge globalThis MessageEvents -> parentPort. Installed lazily on the
// first message listener and torn down when the last one goes away --
// otherwise hasMessageEventListener() in 99_main.js would always be true
// and the worker thread could never exit on its own.
const parentPortBridgeState = new SafeMap<
  MessagePort,
  // deno-lint-ignore no-explicit-any
  { onMessage: (ev: any) => void; onMessageError: (ev: any) => void }
>();

function ensureParentPortBridge(port: MessagePort) {
  if (port[kTransportKind] !== "parent") return;
  if (parentPortBridgeState.has(port)) return;
  // deno-lint-ignore no-explicit-any
  const onMessage = (ev: any) => fireMessage(port, ev.data);
  // deno-lint-ignore no-explicit-any
  const onMessageError = (ev: any) => fireMessageError(port, ev.data);
  globalThis.addEventListener("message", onMessage);
  globalThis.addEventListener("messageerror", onMessageError);
  parentPortBridgeState.set(port, { onMessage, onMessageError });
}

function removeParentPortBridge(port: MessagePort) {
  const entry = parentPortBridgeState.get(port);
  if (!entry) return;
  globalThis.removeEventListener("message", entry.onMessage);
  globalThis.removeEventListener("messageerror", entry.onMessageError);
  parentPortBridgeState.delete(port);
}

// --- MessageChannel ----------------------------------------------------

class _MessageChannelImpl {
  #port1: MessagePort;
  #port2: MessagePort;

  constructor() {
    const { 0: id1, 1: id2 } = op_message_port_create_entangled();
    this.#port1 = createPortFromRid(id1);
    this.#port2 = createPortFromRid(id2);
  }

  get port1(): MessagePort {
    return this.#port1;
  }
  get port2(): MessagePort {
    return this.#port2;
  }
}

// Public `MessageChannel`: a function so that calling it without `new`
// throws `ERR_CONSTRUCT_CALL_REQUIRED` (matching Node), while `new
// MessageChannel()` constructs an _MessageChannelImpl. Prototype chain
// is shared so `instanceof MessageChannel` still works.
function MessageChannel(this: unknown): _MessageChannelImpl {
  if (!new.target) {
    const err = new TypeError(
      "Class constructor MessageChannel cannot be invoked without 'new'",
    );
    // deno-lint-ignore no-explicit-any
    (err as any).code = "ERR_CONSTRUCT_CALL_REQUIRED";
    throw err;
  }
  return new _MessageChannelImpl();
}
MessageChannel.prototype = _MessageChannelImpl.prototype;
ObjectDefineProperty(_MessageChannelImpl.prototype, "constructor", {
  __proto__: null,
  value: MessageChannel,
  writable: true,
  configurable: true,
  enumerable: false,
});

// Public `MessagePort`: same trick -- the class is internal, the
// exported binding is a function that throws ERR_CONSTRUCT_CALL_INVALID
// whether or not `new` is used. `port instanceof MessagePort` and
// `port.constructor === MessagePort` keep working because the function
// shares the class's prototype.
function PublicMessagePort(): never {
  const err = new TypeError(
    "MessagePort cannot be constructed -- use new MessageChannel() instead",
  );
  // deno-lint-ignore no-explicit-any
  (err as any).code = "ERR_CONSTRUCT_CALL_INVALID";
  throw err;
}
PublicMessagePort.prototype = MessagePort.prototype;
ObjectDefineProperty(MessagePort.prototype, "constructor", {
  __proto__: null,
  value: PublicMessagePort,
  writable: true,
  configurable: true,
  enumerable: false,
});

// --- receiveMessageOnPort ----------------------------------------------

function receiveMessageOnPort(port: unknown): { message: unknown } | undefined {
  if (!isMessagePortLike(port)) {
    const err = new TypeError(
      'The "port" argument must be a MessagePort instance',
    );
    // deno-lint-ignore no-explicit-any
    (err as any).code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  // deno-lint-ignore no-explicit-any
  const rid = (port as any)[kPortId];
  if (rid === null) return undefined;
  const data = op_message_port_recv_message_sync(rid);
  if (data === null) return undefined;
  const message = deserializeJsMessageData(data)[0];
  return { message };
}

// --- transferable kind override (take over "MessagePort") --------------

// `send` runs when a MessagePort is included in a transfer list. We
// extract its rid and detach it on this side. Accept both Node and Web
// ports (both store the rid under `MessagePortIdSymbol`).
const nodeMessagePortSend = (port) => {
  const id = port[kPortId];
  port[kPortId] = null;
  if (id === null) {
    throw new DOMException(
      "MessagePort in transfer list is already detached",
      "DataCloneError",
    );
  }
  return id;
};

// `receive` runs on the destination side when a transferred port is
// deserialised. In a Node context we always materialise it as a Node
// MessagePort.
const nodeMessagePortReceive = (rid) => createPortFromRid(rid);

core.registerTransferableResource(
  "MessagePort",
  nodeMessagePortSend,
  nodeMessagePortReceive,
);

// Subset of Node's `internal/worker/io.js` `messageTypes` enum. Only the
// values reached for by `tests/node_compat` are listed; new entries can
// be added as more tests are enabled. Numeric values intentionally match
// the order in Node's source for the curious.
const messageTypes = {
  UP_AND_RUNNING: "upAndRunning",
  COULD_NOT_SERIALIZE_ERROR: "couldNotSerializeError",
  ERROR_MESSAGE: "errorMessage",
  STDIO_PAYLOAD: "stdioPayload",
  STDIO_WANTS_MORE_DATA: "stdioWantsMoreData",
  LOAD_SCRIPT: "loadScript",
};

return {
  MessagePort: PublicMessagePort,
  MessagePortPrototype,
  MessageChannel,
  createParentPort,
  receiveMessageOnPort,
  markAsUntransferable,
  isMarkedAsUntransferable,
  markAsUncloneable,
  messageTypes,
  kPortId,
  kTransportKind,
  kClosed,
};
})();
