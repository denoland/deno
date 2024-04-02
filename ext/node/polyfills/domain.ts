// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6

import { primordials } from "ext:core/mod.js";
const {
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  FunctionPrototypeApply,
} = primordials;
import { EventEmitter } from "node:events";

function emitError(e) {
  console.log(arguments);
  this.emit("error", e);
}

export function create() {
  return new Domain();
}
export class Domain extends EventEmitter {
  constructor() {
    super();
  }

  add(emitter) {
    emitter.on("error", FunctionPrototypeBind(emitError, this));
  }

  remove(emitter) {
    emitter.off("error", FunctionPrototypeBind(emitError, this));
  }

  bind(fn) {
    return function () {
      try {
        FunctionPrototypeApply(fn, null, arguments);
      } catch (e) {
        FunctionPrototypeCall(emitError, this, e);
      }
    };
  }

  intercept(fn) {
    return function (e) {
      if (e) {
        FunctionPrototypeCall(emitError, this, e);
      } else {
        try {
          FunctionPrototypeApply(fn, null, arguments);
        } catch (e) {
          FunctionPrototypeCall(emitError, this, e);
        }
      }
    };
  }

  run(fn) {
    try {
      fn();
    } catch (e) {
      FunctionPrototypeCall(emitError, this, e);
    }
    return this;
  }

  dispose() {
    this.removeAllListeners();
    return this;
  }

  enter() {
    return this;
  }

  exit() {
    return this;
  }
}
export default {
  create,
  Domain,
};
