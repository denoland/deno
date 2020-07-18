// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { customInspect } = window.__bootstrap.console;

  class CountQueuingStrategy {
    constructor({ highWaterMark }) {
      this.highWaterMark = highWaterMark;
    }

    size() {
      return 1;
    }

    [customInspect]() {
      return `${this.constructor.name} { highWaterMark: ${
        String(this.highWaterMark)
      }, size: f }`;
    }
  }

  Object.defineProperty(CountQueuingStrategy.prototype, "size", {
    enumerable: true,
  });

  class ByteLengthQueuingStrategy {
    constructor({ highWaterMark }) {
      this.highWaterMark = highWaterMark;
    }

    size(chunk) {
      return chunk.byteLength;
    }

    [customInspect]() {
      return `${this.constructor.name} { highWaterMark: ${
        String(this.highWaterMark)
      }, size: f }`;
    }
  }

  Object.defineProperty(ByteLengthQueuingStrategy.prototype, "size", {
    enumerable: true,
  });

  window.__bootstrap.queuingStrategy = {
    CountQueuingStrategy,
    ByteLengthQueuingStrategy,
  };
})(this);
