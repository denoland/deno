// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { setEventTargetData } = window.__bootstrap.eventTarget;
  const { defineEventHandler } = window.__bootstrap.event;

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
      webidl.assertBranded(this, MessageChannel);
      return this.#port1;
    }

    get port2() {
      webidl.assertBranded(this, MessageChannel);
      return this.#port2;
    }

    [Symbol.for("Deno.inspect")](inspect) {
      return `MessageChannel ${
        inspect({ port1: this.port1, port2: this.port2 })
      }`;
    }

    get [Symbol.toStringTag]() {
      return "MessageChannel";
    }
  }

  webidl.configurePrototype(MessageChannel);

  const _id = Symbol("id");
  const _enabled = Symbol("enabled");

  /**
   * @param {number} id
   * @returns {MessagePort}
   */
  function createMessagePort(id) {
    const port = webidl.createBranded(MessagePort);
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
     * @param {object[] | PostMessageOptions} transferOrOptions
     */
    postMessage(message, transferOrOptions = {}) {
      webidl.assertBranded(this, MessagePort);
      const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      message = webidl.converters.any(message);
      let options;
      if (
        webidl.type(transferOrOptions) === "Object" &&
        transferOrOptions !== undefined &&
        transferOrOptions[Symbol.iterator] !== undefined
      ) {
        const transfer = webidl.converters["sequence<object>"](
          transferOrOptions,
          { prefix, context: "Argument 2" },
        );
        options = { transfer };
      } else {
        options = webidl.converters.PostMessageOptions(transferOrOptions, {
          prefix,
          context: "Argument 2",
        });
      }
      const { transfer } = options;
      if (transfer.includes(this)) {
        throw new DOMException("Can not tranfer self", "DataCloneError");
      }
      const data = serializeJsMessageData(message, transfer);
      if (this[_id] === null) return;
      core.opSync("op_message_port_post_message", this[_id], data);
    }

    start() {
      webidl.assertBranded(this, MessagePort);
      if (this[_enabled]) return;
      (async () => {
        this[_enabled] = true;
        while (true) {
          if (this[_id] === null) break;
          const data = await core.opAsync(
            "op_message_port_recv_message",
            this[_id],
          );
          if (data === null) break;
          let message, transfer;
          try {
            const v = deserializeJsMessageData(data);
            message = v[0];
            transfer = v[1];
          } catch (err) {
            const event = new MessageEvent("messageerror", { data: err });
            this.dispatchEvent(event);
            return;
          }
          const event = new MessageEvent("message", {
            data: message,
            ports: transfer,
          });
          this.dispatchEvent(event);
        }
        this[_enabled] = false;
      })();
    }

    close() {
      webidl.assertBranded(this, MessagePort);
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

  /**
   * @returns {[number, number]}
   */
  function opCreateEntangledMessagePort() {
    return core.opSync("op_message_port_create_entangled");
  }

  /**
   * @param {globalThis.__bootstrap.messagePort.MessageData} messageData
   * @returns {[any, object[]]}
   */
  function deserializeJsMessageData(messageData) {
    /** @type {object[]} */
    const transferables = [];

    for (const transferable of messageData.transferables) {
      switch (transferable.kind) {
        case "messagePort": {
          const port = createMessagePort(transferable.data);
          transferables.push(port);
          break;
        }
        default:
          throw new TypeError("Unreachable");
      }
    }

    const data = core.deserialize(messageData.data);

    return [data, transferables];
  }

  /**
   * @param {any} data
   * @param {object[]} tranferables
   * @returns {globalThis.__bootstrap.messagePort.MessageData}
   */
  function serializeJsMessageData(data, tranferables) {
    let serializedData;
    try {
      serializedData = core.serialize(data);
    } catch (err) {
      throw new DOMException(err.message, "DataCloneError");
    }

    /** @type {globalThis.__bootstrap.messagePort.Transferable[]} */
    const serializedTransferables = [];

    for (const transferable of tranferables) {
      if (transferable instanceof MessagePort) {
        webidl.assertBranded(transferable, MessagePort);
        const id = transferable[_id];
        if (id === null) {
          throw new DOMException(
            "Can not transfer disentangled message port",
            "DataCloneError",
          );
        }
        transferable[_id] = null;
        serializedTransferables.push({ kind: "messagePort", data: id });
      } else {
        throw new DOMException("Value not transferable", "DataCloneError");
      }
    }

    return {
      data: serializedData,
      transferables: serializedTransferables,
    };
  }

  webidl.converters.PostMessageOptions = webidl.createDictionaryConverter(
    "PostMessageOptions",
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

  window.__bootstrap.messagePort = {
    MessageChannel,
    MessagePort,
    deserializeJsMessageData,
    serializeJsMessageData,
  };
})(globalThis);
