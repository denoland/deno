// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  /**
   * https://infra.spec.whatwg.org/#collect-a-sequence-of-code-points
   * @param {string} input
   * @param {number} position
   * @param {(char: string) => boolean} condition
   * @returns {{result: string, position: number}}
   */
  function collectSequenceOfCodepoints(input, position, condition) {
    const start = position;
    for (
      let c = input.charAt(position);
      position < input.length && condition(c);
      c = input.charAt(++position)
    );
    return { result: input.slice(start, position), position };
  }

  window.__bootstrap.infra = {
    collectSequenceOfCodepoints,
  };
})(globalThis);
