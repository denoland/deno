// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

import { core, primordials } from "ext:core/mod.js";
import {
  op_message_port_create_entangled,
  op_message_port_post_message,
  op_message_port_recv_message,
} from "ext:core/ops";
const {
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypePush,
  ObjectPrototypeIsPrototypeOf,
  ObjectDefineProperty,
  Symbol,
  SymbolFor,
  SymbolIterator,
  PromiseResolve,
  SafeArrayIterator,
  TypeError,
} = primordials;
const {
  InterruptedPrototype,
  isArrayBuffer,
} = core;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import {
  defineEventHandler,
  EventTarget,
  MessageEvent,
  setEventTargetData,
  setIsTrusted,
} from "./02_event.js";
import { isDetachedBuffer } from "./06_streams.js";
import { DOMException } from "./01_dom_exception.js";

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
      createFilteredInspectProxy({
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
export const refMessagePort = Symbol("refMessagePort");
/** It is used by 99_main.js and worker_threads to
 * unref/ref on the global pollForMessages promise. */
export const unrefPollForMessages = Symbol("unrefPollForMessages");

/**
 * @param {number} id
 * @returns {MessagePort}
 */
function createMessagePort(id) {
  const port = webidl.createBranded(MessagePort);
  port[core.hostObjectBrand] = core.hostObjectBrand;
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

class MessagePort extends EventTarget {
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
    webidl.requiredArguments(arguments.length, 1, prefix);
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
    const { transfer } = options;
    if (ArrayPrototypeIncludes(transfer, this)) {
      throw new DOMException("Can not transfer self", "DataCloneError");
    }
    const data = serializeJsMessageData(message, transfer);
    if (this[_id] === null) return;
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
        let message, transferables;
        try {
          const v = deserializeJsMessageData(data);
          message = v[0];
          transferables = v[1];
        } catch (err) {
          const event = new MessageEvent("messageerror", { data: err });
          setIsTrusted(event, true);
          this.dispatchEvent(event);
          return;
        }
        const event = new MessageEvent("message", {
          data: message,
          ports: ArrayPrototypeFilter(
            transferables,
            (t) => ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t),
          ),
        });
        setIsTrusted(event, true);
        this.dispatchEvent(event);
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

  close() {
    webidl.assertBranded(this, MessagePortPrototype);
    if (this[_id] !== null) {
      core.close(this[_id]);
      this[_id] = null;
      nodeWorkerThreadMaybeInvokeCloseCb(this);
    }
  }

  removeEventListener(...args) {
    if (args[0] == "message") {
      if (--this[_messageEventListenerCount] === 0 && this[_refed]) {
        refedMessagePortsCount--;
        this[_refed] = false;
      }
    }
    super.removeEventListener(...new SafeArrayIterator(args));
  }

  addEventListener(...args) {
    if (args[0] == "message") {
      if (++this[_messageEventListenerCount] === 1 && !this[_refed]) {
        refedMessagePortsCount++;
        this[_refed] = true;
      }
    }
    super.addEventListener(...new SafeArrayIterator(args));
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(MessagePortPrototype, this),
        keys: [
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
function deserializeJsMessageData(messageData) {
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
        case "messagePort": {
          const port = createMessagePort(transferable.data);
          ArrayPrototypePush(transferables, port);
          ArrayPrototypePush(hostObjects, port);
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
function serializeJsMessageData(data, transferables) {
  let options;
  const transferredArrayBuffers = [];
  if (transferables.length > 0) {
    const hostObjects = [];
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
      } else if (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t)) {
        ArrayPrototypePush(hostObjects, t);
      }
    }

    options = {
      hostObjects,
      transferredArrayBuffers,
    };
  }

  const serializedData = core.serialize(data, options, (err) => {
    throw new DOMException(err, "DataCloneError");
  });

  /** @type {messagePort.Transferable[]} */
  const serializedTransferables = [];

  let arrayBufferI = 0;
  for (let i = 0; i < transferables.length; ++i) {
    const transferable = transferables[i];
    if (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, transferable)) {
      webidl.assertBranded(transferable, MessagePortPrototype);
      const id = transferable[_id];
      if (id === null) {
        throw new DOMException(
          "Can not transfer disentangled message port",
          "DataCloneError",
        );
      }
      transferable[_id] = null;
      ArrayPrototypePush(serializedTransferables, {
        kind: "messagePort",
        data: id,
      });
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

function structuredClone(value, options) {
  const prefix = "Failed to execute 'structuredClone'";
  webidl.requiredArguments(arguments.length, 1, prefix);
  options = webidl.converters.StructuredSerializeOptions(
    options,
    prefix,
    "Argument 2",
  );
  const messageData = serializeJsMessageData(value, options.transfer);
  return deserializeJsMessageData(messageData)[0];
}

export {
  deserializeJsMessageData,
  MessageChannel,
  MessagePort,
  MessagePortIdSymbol,
  MessagePortPrototype,
  MessagePortReceiveMessageOnPortSymbol,
  nodeWorkerThreadCloseCb,
  refedMessagePortsCount,
  serializeJsMessageData,
  structuredClone,
};
