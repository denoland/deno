// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

// Transport-only module: ports / channels are exposed by
// `node:worker_threads` (see ext/node/polyfills/internal/worker/io.ts),
// which is responsible for the `globalThis.MessagePort` /
// `globalThis.MessageChannel` constructors. This file owns the
// underlying ops, the serialize/deserialize round-trip, and the
// non-port pieces of `structuredClone`.
(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeIncludes,
  ArrayPrototypePush,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  Symbol,
  SymbolFor,
  TypeError,
  TypeErrorPrototype,
} = primordials;
const { isArrayBuffer } = core;
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");

const {
  ReadableStream,
  WritableStream,
  TransformStream,
} = core.loadExtScript("ext:deno_web/06_streams.js");

const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

// Shared port-id symbol -- the Node MessagePort uses this slot directly
// (imported as `kPortId`) so a transferred rid lands on the same field
// regardless of which side allocated the wrapper.
const MessagePortIdSymbol = Symbol("MessagePortId");

// Used by 99_main.js and node:worker_threads to flip the "treat
// parentPort listeners as if they were absent" bit on the worker's
// globalThis, so `parentPort.unref()` can let the worker exit even
// while message listeners stay registered.
const unrefParentPort = Symbol("unrefParentPort");

// Shared marker symbol with ext/node's `markAsUncloneable`. Lives in
// the global Symbol registry so both extensions reference the same
// symbol without needing a cross-extension import.
const kNodeUncloneable = SymbolFor("nodejs.worker_threads.uncloneable");

/**
 * @param {messagePort.MessageData} messageData
 * @returns {[any, object[]]}
 */
const emptyTransferables = ObjectFreeze([]);

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
          const hostObj = core.getTransferableResource(type).receive(rid);
          ArrayPrototypePush(transferables, hostObj);
          ArrayPrototypePush(hostObjects, hostObj);
          break;
        }
        case "multiResource": {
          const { 0: type, 1: rids } = transferable.data;
          const hostObj = core.getTransferableResource(type).receive(rids);
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
  // V8 reports a generic "Unsupported object type" for host objects it
  // doesn't know how to clone. The most common cause inside Node.js
  // code is a `MessagePort` in the message but missing from the
  // transfer list. Translate to the wording Node uses so tests that
  // pattern-match on that exact string pass.
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
      const rid = core.getTransferableResource(type).send(transferable);
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

// Streams are defined in this extension, so mark them here. Fetch types
// (Headers / Request / Response) call `markNotSerializable` themselves at
// the bottom of their respective modules.
markNotSerializable(ReadableStream.prototype);
markNotSerializable(WritableStream.prototype);
markNotSerializable(TransformStream.prototype);

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
  webidl.requiredArguments(arguments.length, 1, prefix);
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
    value !== null && typeof value === "object" && value[kNotSerializable] &&
    !ArrayPrototypeIncludes(options.transfer, value)
  ) {
    throw new DOMException(
      "Cannot clone object of unsupported type.",
      "DataCloneError",
    );
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
  kNodeUncloneable,
  kNotSerializable,
  markNotSerializable,
  MessagePortIdSymbol,
  serializeJsMessageData,
  structuredClone,
  unrefParentPort,
};
})();
