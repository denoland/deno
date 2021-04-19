// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const ASCII_DIGIT = ["\u0030-\u0039"];
  const ASCII_UPPER_ALPHA = ["\u0041-\u005A"];
  const ASCII_LOWER_ALPHA = ["\u0061-\u007A"];
  const ASCII_ALPHA = [...ASCII_UPPER_ALPHA, ...ASCII_LOWER_ALPHA];
  const ASCII_ALPHANUMERIC = [...ASCII_DIGIT, ...ASCII_ALPHA];

  const HTTP_TAB_OR_SPACE = ["\u0009", "\u0020"];
  const HTTP_WHITESPACE = ["\u000A", "\u000D", ...HTTP_TAB_OR_SPACE];

  const HTTP_TOKEN_CODE_POINT = [
    "\u0021",
    "\u0023",
    "\u0024",
    "\u0025",
    "\u0026",
    "\u0027",
    "\u002A",
    "\u002B",
    "\u002D",
    "\u002E",
    "\u005E",
    "\u005F",
    "\u0060",
    "\u007C",
    "\u007E",
    ...ASCII_ALPHANUMERIC,
  ];
  const HTTP_TOKEN_CODE_POINT_RE = new RegExp(
    `^[${regexMatcher(HTTP_TOKEN_CODE_POINT)}]+$`,
  );
  const HTTP_QUOTED_STRING_TOKEN_POINT = [
    "\u0009",
    "\u0020-\u007E",
    "\u0080-\u00FF",
  ];
  const HTTP_QUOTED_STRING_TOKEN_POINT_RE = new RegExp(
    `^[${regexMatcher(HTTP_QUOTED_STRING_TOKEN_POINT)}]+$`,
  );
  const HTTP_TAB_OR_SPACE_MATCHER = regexMatcher(HTTP_TAB_OR_SPACE);
  const HTTP_TAB_OR_SPACE_PREFIX_RE = new RegExp(
    `^[${HTTP_TAB_OR_SPACE_MATCHER}]+`,
    "g",
  );
  const HTTP_TAB_OR_SPACE_SUFFIX_RE = new RegExp(
    `[${HTTP_TAB_OR_SPACE_MATCHER}]+$`,
    "g",
  );
  const HTTP_WHITESPACE_MATCHER = regexMatcher(HTTP_WHITESPACE);
  const HTTP_WHITESPACE_PREFIX_RE = new RegExp(
    `^[${HTTP_WHITESPACE_MATCHER}]+`,
    "g",
  );
  const HTTP_WHITESPACE_SUFFIX_RE = new RegExp(
    `[${HTTP_WHITESPACE_MATCHER}]+$`,
    "g",
  );

  /**
   * Turn a string of chars into a regex safe matcher.
   * @param {string[]} chars 
   * @returns {string}
   */
  function regexMatcher(chars) {
    const matchers = chars.map((char) => {
      if (char.length === 1) {
        return `\\u${char.charCodeAt(0).toString(16).padStart(4, "0")}`;
      } else if (char.length === 3 && char[1] === "-") {
        return `\\u${char.charCodeAt(0).toString(16).padStart(4, "0")}-\\u${
          char.charCodeAt(2).toString(16).padStart(4, "0")
        }`;
      } else {
        throw TypeError("unreachable");
      }
    });
    return matchers.join("");
  }

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

  /**
   * @param {string} s
   * @returns {string}
   */
  function byteUpperCase(s) {
    return String(s).replace(/[a-z]/g, function byteUpperCaseReplace(c) {
      return c.toUpperCase();
    });
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function byteLowerCase(s) {
    return String(s).replace(/[A-Z]/g, function byteUpperCaseReplace(c) {
      return c.toLowerCase();
    });
  }

  /**
   * https://fetch.spec.whatwg.org/#collect-an-http-quoted-string
   * @param {string} input
   * @param {number} position
   * @param {boolean} extractValue
   * @returns {{result: string, position: number}}
   */
  function collectHttpQuotedString(input, position, extractValue) {
    // 1.
    const positionStart = position;
    // 2.
    let value = "";
    // 3.
    if (input[position] !== "\u0022") throw new Error('must be "');
    // 4.
    position++;
    // 5.
    while (true) {
      // 5.1.
      const res = collectSequenceOfCodepoints(
        input,
        position,
        (c) => c !== "\u0022" && c !== "\u005C",
      );
      value += res.result;
      position = res.position;
      // 5.2.
      if (position >= input.length) break;
      // 5.3.
      const quoteOrBackslash = input[position];
      // 5.4.
      position++;
      // 5.5.
      if (quoteOrBackslash === "\u005C") {
        // 5.5.1.
        if (position >= input.length) {
          value += "\u005C";
          break;
        }
        // 5.5.2.
        value += input[position];
        // 5.5.3.
        position++;
      } else { // 5.6.
        // 5.6.1
        if (quoteOrBackslash !== "\u0022") throw new Error('must be "');
        // 5.6.2
        break;
      }
    }
    // 6.
    if (extractValue) return { result: value, position };
    // 7.
    return { result: input.substring(positionStart, position + 1), position };
  }

  window.__bootstrap.infra = {
    collectSequenceOfCodepoints,
    ASCII_DIGIT,
    ASCII_UPPER_ALPHA,
    ASCII_LOWER_ALPHA,
    ASCII_ALPHA,
    ASCII_ALPHANUMERIC,
    HTTP_TAB_OR_SPACE,
    HTTP_WHITESPACE,
    HTTP_TOKEN_CODE_POINT,
    HTTP_TOKEN_CODE_POINT_RE,
    HTTP_QUOTED_STRING_TOKEN_POINT,
    HTTP_QUOTED_STRING_TOKEN_POINT_RE,
    HTTP_TAB_OR_SPACE_PREFIX_RE,
    HTTP_TAB_OR_SPACE_SUFFIX_RE,
    HTTP_WHITESPACE_PREFIX_RE,
    HTTP_WHITESPACE_SUFFIX_RE,
    regexMatcher,
    byteUpperCase,
    byteLowerCase,
    collectHttpQuotedString,
  };
})(globalThis);
