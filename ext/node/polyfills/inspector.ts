// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import {
  op_create_inspector_session,
  op_inspector_get_message_from_v8,
  op_inspector_post,
} from "ext:core/ops";
import {
  ERR_INSPECTOR_ALREADY_CONNECTED,
  ERR_INSPECTOR_COMMAND,
  ERR_INSPECTOR_NOT_CONNECTED,
} from "ext:deno_node/internal/errors.ts";
import {
  validateFunction,
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { EventEmitter } from "node:events";
import { emitWarning } from "node:process";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { core, primordials } from "ext:core/mod.js";

const {
  SafeMap,
  JSONParse,
} = primordials;

class Session extends EventEmitter {
  #connection: number | null = null;
  #nextId = 1;
  #messageCallbacks = new SafeMap();

  /** Connects the session to the inspector back-end. */
  connect(): void {
    if (this.#connection) {
      throw new ERR_INSPECTOR_ALREADY_CONNECTED("The inspector session");
    }

    // this.#connection = TODO;
    this.#connection = 1;
    op_create_inspector_session();

    // Start listening for messages - this is using "unrefed" op
    // so that listening for notifications doesn't block the event loop.
    // When posting a message another promise should be started that is refed.
    (async () => {
      while (true) {
        await this.#listenForMessage(true);
      }
    })();
  }

  async #listenForMessage(unref: boolean) {
    let message: string;
    try {
      const opPromise = op_inspector_get_message_from_v8();
      if (unref) {
        core.unrefOpPromise(opPromise);
      }
      message = await opPromise;
      this.#onMessage(message);
    } catch (e) {
      emitWarning(e);
    }
  }

  #onMessage(message: string) {
    core.print("received message" + message + "\n", true);
    const parsed = JSONParse(message);
    if (parsed.id) {
      const callback = this.#messageCallbacks.get(parsed.id);
      this.#messageCallbacks.delete(parsed.id);
      if (callback) {
        if (parsed.error) {
          return callback(
            new ERR_INSPECTOR_COMMAND(parsed.error.code, parsed.error.message),
          );
        }

        callback(null, parsed.result);
      }
    } else {
      this.emit(parsed.method, parsed);
      this.emit("inspectorNotification", parsed);
    }
  }

  /** Connects the session to the main thread
   * inspector back-end. */
  connectToMainThread(): void {
    notImplemented("inspector.Session.prototype.connectToMainThread");
  }

  /** Posts a message to the inspector back-end. */
  post(
    method: string,
    params: Record<string, unknown> | null,
    callback?: (...args: unknown[]) => void,
  ): void {
    validateString(method, "method");
    if (!callback && typeof params === "function") {
      callback = params;
      params = null;
    }
    if (params) {
      validateObject(params, "params");
    }
    if (callback) {
      validateFunction(callback, "callback");
    }

    if (!this.#connection) {
      throw new ERR_INSPECTOR_NOT_CONNECTED();
    }
    const id = this.#nextId++;
    if (callback) {
      this.#messageCallbacks.set(id, callback);
    }
    console.log("posting message");
    // TODO(bartlomieju): Ignore errors?
    op_inspector_post(id, method, params);
    this.#listenForMessage(false);
  }

  /** Immediately closes the session, all pending
   * message callbacks will be called with an
   * error.
   */
  disconnect() {
    notImplemented("inspector.Session.prototype.disconnect");
  }
}

/** Activates inspector on host and port.
 * See https://nodejs.org/api/inspector.html#inspectoropenport-host-wait */
function open(_port?: number, _host?: string, _wait?: boolean) {
  notImplemented("inspector.Session.prototype.open");
}

/** Deactivate the inspector. Blocks until there are no active connections.
 * See https://nodejs.org/api/inspector.html#inspectorclose */
function close() {
  notImplemented("inspector.Session.prototype.close");
}

/** Return the URL of the active inspector, or undefined if there is none.
 * See https://nodejs.org/api/inspector.html#inspectorurl */
function url() {
  // TODO(kt3k): returns undefined for now, which means the inspector is not activated.
  return undefined;
}

/** Blocks until a client (existing or connected later) has sent Runtime.runIfWaitingForDebugger command.
 * See https://nodejs.org/api/inspector.html#inspectorwaitfordebugger */
function waitForDebugger() {
  notImplemented("inspector.wairForDebugger");
}

const console = globalThis.console;

export { close, console, open, Session, url, waitForDebugger };

export default {
  close,
  console,
  open,
  Session,
  url,
  waitForDebugger,
};
