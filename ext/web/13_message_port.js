// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const { InterruptedPrototype, ops } = core;
  const webidl = window.__bootstrap.webidl;
  const { setEventTargetData } = window.__bootstrap.eventTarget;
  const { defineEventHandler } = window.__bootstrap.event;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayBufferPrototype,
    ArrayPrototypeFilter,
    ArrayPrototypeIncludes,
    ArrayPrototypePush,
    ObjectPrototypeIsPrototypeOf,
    ObjectSetPrototypeOf,
    Symbol,
    SymbolFor,
    SymbolIterator,
    TypeError,
    WeakSet,
    WeakSetPrototypeAdd,
    WeakSetPrototypeHas,
  } = window.__bootstrap.primordials;

  class MessageChannel {
    /** @type {MessagePort} */
    #port1;
    /** @type {MessagePort} */
    #port2;

    constructor() {
      this[webidl.brand] = webidl.brand;
      const [port1Id, port2Id] = opCreateEntangledMessagePort();
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

    [SymbolFor("Deno.inspect")](inspect) {
      return `MessageChannel ${
        inspect({ port1: this.port1, port2: this.port2 })
      }`;
    }
  }

  webidl.configurePrototype(MessageChannel);
  const MessageChannelPrototype = MessageChannel.prototype;

  const _id = Symbol("id");
  const _enabled = Symbol("enabled");

  /**
   * @param {number} id
   * @returns {MessagePort}
   */
  function createMessagePort(id) {
    const port = core.createHostObject();
    ObjectSetPrototypeOf(port, MessagePortPrototype);
    port[webidl.brand] = webidl.brand;
    setEventTargetData(port);
    port[_id] = id;
    return port;
  }

  class MessagePort extends EventTarget {
    /** @type {number | null} */
    [_id] = null;
    /** @type {boolean} */
    [_enabled] = false;

    constructor() {
      super();
      webidl.illegalConstructor();
    }

    /**
     * @param {any} message
     * @param {object[] | StructuredSerializeOptions} transferOrOptions
     */
    postMessage(message, transferOrOptions = {}) {
      webidl.assertBranded(this, MessagePortPrototype);
      const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      message = webidl.converters.any(message);
      let options;
      if (
        webidl.type(transferOrOptions) === "Object" &&
        transferOrOptions !== undefined &&
        transferOrOptions[SymbolIterator] !== undefined
      ) {
        const transfer = webidl.converters["sequence<object>"](
          transferOrOptions,
          { prefix, context: "Argument 2" },
        );
        options = { transfer };
      } else {
        options = webidl.converters.StructuredSerializeOptions(
          transferOrOptions,
          {
            prefix,
            context: "Argument 2",
          },
        );
      }
      const { transfer } = options;
      if (ArrayPrototypeIncludes(transfer, this)) {
        throw new DOMException("Can not tranfer self", "DataCloneError");
      }
      const data = serializeJsMessageData(message, transfer);
      if (this[_id] === null) return;
      ops.op_message_port_post_message(this[_id], data);
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
            data = await core.opAsync(
              "op_message_port_recv_message",
              this[_id],
            );
          } catch (err) {
            if (ObjectPrototypeIsPrototypeOf(InterruptedPrototype, err)) break;
            throw err;
          }
          if (data === null) break;
          let message, transferables;
          try {
            const v = deserializeJsMessageData(data);
            message = v[0];
            transferables = v[1];
          } catch (err) {
            const event = new MessageEvent("messageerror", { data: err });
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
          this.dispatchEvent(event);
        }
        this[_enabled] = false;
      })();
    }

    close() {
      webidl.assertBranded(this, MessagePortPrototype);
      if (this[_id] !== null) {
        core.close(this[_id]);
        this[_id] = null;
      }
    }
  }

  defineEventHandler(MessagePort.prototype, "message", function (self) {
    self.start();
  });
  defineEventHandler(MessagePort.prototype, "messageerror");

  webidl.configurePrototype(MessagePort);
  const MessagePortPrototype = MessagePort.prototype;

  /**
   * @returns {[number, number]}
   */
  function opCreateEntangledMessagePort() {
    return ops.op_message_port_create_entangled();
  }

  /**
   * @param {globalThis.__bootstrap.messagePort.MessageData} messageData
   * @returns {[any, object[]]}
   */
  function deserializeJsMessageData(messageData) {
    /** @type {object[]} */
    const transferables = [];
    const hostObjects = [];
    const arrayBufferIdsInTransferables = [];
    const transferredArrayBuffers = [];

    for (const transferable of messageData.transferables) {
      switch (transferable.kind) {
        case "messagePort": {
          const port = createMessagePort(transferable.data);
          ArrayPrototypePush(transferables, port);
          ArrayPrototypePush(hostObjects, port);
          break;
        }
        case "arrayBuffer": {
          ArrayPrototypePush(transferredArrayBuffers, transferable.data);
          const i = ArrayPrototypePush(transferables, null);
          ArrayPrototypePush(arrayBufferIdsInTransferables, i);
          break;
        }
        default:
          throw new TypeError("Unreachable");
      }
    }

    const data = core.deserialize(messageData.data, {
      hostObjects,
      transferredArrayBuffers,
    });

    for (const i in arrayBufferIdsInTransferables) {
      const id = arrayBufferIdsInTransferables[i];
      transferables[id] = transferredArrayBuffers[i];
    }

    return [data, transferables];
  }

  const detachedArrayBuffers = new WeakSet();

  /**
   * @param {any} data
   * @param {object[]} transferables
   * @returns {globalThis.__bootstrap.messagePort.MessageData}
   */
  function serializeJsMessageData(data, transferables) {
    const transferredArrayBuffers = ArrayPrototypeFilter(
      transferables,
      (a) => ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, a),
    );

    for (const arrayBuffer of transferredArrayBuffers) {
      // This is hacky with both false positives and false negatives for
      // detecting detached array buffers. V8  needs to add a way to tell if a
      // buffer is detached or not.
      if (WeakSetPrototypeHas(detachedArrayBuffers, arrayBuffer)) {
        throw new DOMException(
          "Can not transfer detached ArrayBuffer",
          "DataCloneError",
        );
      }
      WeakSetPrototypeAdd(detachedArrayBuffers, arrayBuffer);
    }

    const serializedData = core.serialize(data, {
      hostObjects: ArrayPrototypeFilter(
        transferables,
        (a) => ObjectPrototypeIsPrototypeOf(MessagePortPrototype, a),
      ),
      transferredArrayBuffers,
    }, (err) => {
      throw new DOMException(err, "DataCloneError");
    });

    /** @type {globalThis.__bootstrap.messagePort.Transferable[]} */
    const serializedTransferables = [];

    let arrayBufferI = 0;
    for (const transferable of transferables) {
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
      } else if (
        ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, transferable)
      ) {
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
    webidl.requiredArguments(arguments.length, 1, { prefix });
    options = webidl.converters.StructuredSerializeOptions(options, {
      prefix,
      context: "Argument 2",
    });
    const messageData = serializeJsMessageData(value, options.transfer);
    const [data] = deserializeJsMessageData(messageData);
    return data;
  }

  window.__bootstrap.messagePort = {
    MessageChannel,
    MessagePort,
    MessagePortPrototype,
    deserializeJsMessageData,
    serializeJsMessageData,
    structuredClone,
  };
})(globalThis);
