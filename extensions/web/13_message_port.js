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

  const _id = Symbol("id");

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
    [_id];

    constructor() {
      super();
      webidl.illegalConstructor();
    }

    /**
     * @param {any} message
     * @param {object[] | PostMessageOptions} transferOrOptions
     */
    postMessage(message, transferOrOptions = {}) {
      let transfer = [];
      if (Array.isArray(transferOrOptions)) {
        transfer = transferOrOptions;
      } else if (Array.isArray(transferOrOptions.transfer)) {
        transfer = transferOrOptions.transfer;
      }

      const data = serializeJsMessageData(message, transfer);
      core.opSync("op_message_port_post_message", this[_id], data);
    }

    start() {
      (async () => {
        while (true) {
          const data = await core.opAsync(
            "op_message_port_recv_message",
            this[_id],
          );
          if (data === null) break;
          const [message, transfer] = deserializeJsMessageData(data);
          const event = new MessageEvent("message", {
            data: message,
            ports: transfer,
          });
          this.dispatchEvent(event);
        }
      })();
    }

    close() {
      if (this[_id] !== null) {
        core.close(this[_id]);
      }
    }
  }

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
    const serializedData = core.serialize(data);

    /** @type {globalThis.__bootstrap.messagePort.Transferable[]} */
    const serializedTransferables = [];

    for (const transferable of tranferables) {
      if (transferable instanceof MessagePort) {
        webidl.assertBranded(transferable, MessagePort);
        const id = transferable[_id];
        if (id === null) {
          throw new TypeError("Can not transfer disentangled message port");
        }
        serializedTransferables.push({ kind: "messagePort", data: id });
      } else {
        throw new TypeError("Value not transferable");
      }
    }

    return {
      data: serializedData,
      transferables: serializedTransferables,
    };
  }

  window.__bootstrap.messagePort = {
    MessageChannel,
    MessagePort,
    deserializeJsMessageData,
    serializeJsMessageData,
  };
})(globalThis);
