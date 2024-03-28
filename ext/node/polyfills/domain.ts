// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6

import { EventEmitter } from "node:events";

function emitError(e) {
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
    emitter.on("error", emitError.bind(this));
  }

  remove(emitter) {
    emitter.removeListener("error", emitError.bind(this));
  }

  bind(fn) {
    return function () {
      try {
        fn.apply(null, arguments);
      } catch (e) {
        emitError.call(this, e);
      }
    };
  }

  intercept(fn) {
    return function (e) {
      if (e) {
        emitError.call(this, e);
      } else {
        try {
          fn.apply(null, arguments);
        } catch (e) {
          emitError.call(this, e);
        }
      }
    };
  }

  run(fn) {
    try {
      fn();
    } catch (e) {
      emitError.call(this, e);
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
