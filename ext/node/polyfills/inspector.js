// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import process from "node:process";
import { EventEmitter } from "node:events";
import { core, primordials } from "ext:core/mod.js";
import {
  op_get_extras_binding_object,
  op_inspector_open,
  op_inspector_connect,
  op_inspector_dispatch,
  op_inspector_disconnect,
  op_inspector_close,
  op_inspector_wait,
} from "ext:core/ops";
import {
    isUint32,
  validateFunction,
  validateInt32,
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import {
  ERR_INSPECTOR_ALREADY_ACTIVATED,
  ERR_INSPECTOR_ALREADY_CONNECTED,
  ERR_INSPECTOR_CLOSED,
  ERR_INSPECTOR_COMMAND,
  ERR_INSPECTOR_NOT_CONNECTED,
  ERR_INSPECTOR_NOT_ACTIVE,
  ERR_INSPECTOR_NOT_WORKER,
} from "ext:deno_node/internal/errors.ts";

const {
  SymbolDispose,
  JSONParse,
  JSONStringify,
  SafeMap,
} = primordials;

class Session extends EventEmitter {
  #connection = null;
  #nextId = 1;
  #messageCallbacks = new SafeMap();

  connect() {
    if (this.#connection)
      throw new ERR_INSPECTOR_ALREADY_CONNECTED('The inspector session');
    this.#connection = op_inspector_connect((message) => this.#onMessage(message), false);
  }

  connectToMainThread() {
    if (isMainThread)
      throw new ERR_INSPECTOR_NOT_WORKER();
    if (this.#connection)
      throw new ERR_INSPECTOR_ALREADY_CONNECTED('The inspector session');
    this.#connection = op_inspector_connect((message) => queueMicrotask(() => this.#onMessage(message)), true);
  }

  #onMessage() {
    const parsed = JSONParse(message);
    try {
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
        this.emit('inspectorNotification', parsed);
      }
    } catch (error) {
      process.emitWarning(error);
    }
  }

  post(method, params, callback) {
    validateString(method, 'method');
    if (!callback && typeof params === 'function') {
      callback = params;
      params = null;
    }
    if (params) {
      validateObject(params, 'params');
    }
    if (callback) {
      validateFunction(callback, 'callback');
    }

    if (!this.#connection) {
      throw new ERR_INSPECTOR_NOT_CONNECTED();
    }
    const id = this.#nextId++;
    const message = { id, method };
    if (params) {
      message.params = params;
    }
    if (callback) {
      this.#messageCallbacks.set(id, callback);
    }
    op_inspector_dispatch(this.#connection, JSONStringify(message));
  }

  disconnect() {
    if (!this.#connection)
      return;
    op_inspector_disconnect(this.#connection);
    this.#connection = null;
    const remainingCallbacks = this.#messageCallbacks.values();
    for (const callback of remainingCallbacks) {
      process.nextTick(callback, new ERR_INSPECTOR_CLOSED());
    }
    this.#messageCallbacks.clear();
    this.#nextId = 1;
  }
}

function open(port, host, wait) {
  if (isEnabled()) {
    throw new ERR_INSPECTOR_ALREADY_ACTIVATED();
  }
  // inspectorOpen() currently does not typecheck its arguments and adding
  // such checks would be a potentially breaking change. However, the native
  // open() function requires the port to fit into a 16-bit unsigned integer,
  // causing an integer overflow otherwise, so we at least need to prevent that.
  if (isUint32(port)) {
    validateInt32(port, 'port', 0, 65535);
  }
  op_inspector_open(port, host);
  if (wait)
    op_inspector_wait();

  return { __proto__: null, [SymbolDispose]() { _debugEnd(); } };
}

function close() {
  op_inspector_close();
}

function url() {
  return op_inspector_url();
}

function waitForDebugger() {
  if (!op_inspector_wait())
    throw new ERR_INSPECTOR_NOT_ACTIVE();
}

function broadcastToFrontend(eventName, params) {
  validateString(eventName, 'eventName');
  if (params) {
    validateObject(params, 'params');
  }
  emitProtocolEvent(eventName, JSONStringify(params ?? {}));
}

const Network = {
  requestWillBeSent: (params) => broadcastToFrontend('Network.requestWillBeSent', params),
  responseReceived: (params) => broadcastToFrontend('Network.responseReceived', params),
  loadingFinished: (params) => broadcastToFrontend('Network.loadingFinished', params),
  loadingFailed: (params) => broadcastToFrontend('Network.loadingFailed', params),
};

const console = op_get_extras_binding_object().console;

export {
  open,
  close,
  url,
  waitForDebugger,
  console,
  Session,
  Network,
};

export default {
  open,
  close,
  url,
  waitForDebugger,
  console,
  Session,
  Network,
};
