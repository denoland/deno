// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/worker.js.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { core, primordials } from "ext:core/mod.js";
import { EventEmitter } from "node:events";
const { kEmptyObject } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);

const { FunctionPrototypeCall, ObjectSetPrototypeOf, ReflectApply } =
  primordials;

// Common Worker implementation shared between cluster primary and worker.
export function Worker(this: any, options?: any) {
  if (!(this instanceof Worker)) {
    return new (Worker as any)(options);
  }

  FunctionPrototypeCall(EventEmitter, this);

  if (options === null || typeof options !== "object") {
    options = kEmptyObject;
  }

  this.exitedAfterDisconnect = undefined;

  this.state = options.state || "none";
  this.id = options.id | 0;

  if (options.process) {
    this.process = options.process;
    this.process.on(
      "error",
      (code: any, signal: any) => this.emit("error", code, signal),
    );
    this.process.on(
      "message",
      (message: any, handle: any) => this.emit("message", message, handle),
    );
  }
}

ObjectSetPrototypeOf(Worker.prototype, EventEmitter.prototype);
ObjectSetPrototypeOf(Worker, EventEmitter);

(Worker as any).prototype.kill = function (this: any) {
  ReflectApply(this.destroy, this, arguments);
};

(Worker as any).prototype.send = function (this: any) {
  return ReflectApply(this.process.send, this.process, arguments);
};

(Worker as any).prototype.isDead = function (this: any) {
  return this.process.exitCode != null || this.process.signalCode != null;
};

(Worker as any).prototype.isConnected = function (this: any) {
  return this.process.connected;
};

export default Worker;
