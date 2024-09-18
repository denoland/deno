// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { EventEmitter } from "node:events";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { primordials } from "ext:core/mod.js";

const {
  SafeMap,
} = primordials;

class Session extends EventEmitter {
  #connection = null;
  #nextId = 1;
  #messageCallbacks = new SafeMap();

  /** Connects the session to the inspector back-end. */
  connect() {
    notImplemented("inspector.Session.prototype.connect");
  }

  /** Connects the session to the main thread
   * inspector back-end. */
  connectToMainThread() {
    notImplemented("inspector.Session.prototype.connectToMainThread");
  }

  /** Posts a message to the inspector back-end. */
  post(
    _method: string,
    _params?: Record<string, unknown>,
    _callback?: (...args: unknown[]) => void,
  ) {
    notImplemented("inspector.Session.prototype.post");
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
