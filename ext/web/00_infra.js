// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = Deno.core;
  const ops = core.ops;
  const {
    ArrayPrototypeJoin,
    ArrayPrototypeMap,
    Error,
    JSONStringify,
    NumberPrototypeToString,
    RegExp,
    SafeArrayIterator,
    String,
    StringPrototypeCharAt,
    StringPrototypeCharCodeAt,
    StringPrototypeMatch,
    StringPrototypePadStart,
    StringPrototypeReplace,
    StringPrototypeSlice,
    StringPrototypeSubstring,
    StringPrototypeToLowerCase,
    StringPrototypeToUpperCase,
    TypeError,
  } = window.__bootstrap.primordials;

  const ASCII_DIGIT = ["\u0030-\u0039"];
  const ASCII_UPPER_ALPHA = ["\u0041-\u005A"];
  const ASCII_LOWER_ALPHA = ["\u0061-\u007A"];
  const ASCII_ALPHA = [
    ...new SafeArrayIterator(ASCII_UPPER_ALPHA),
    ...new SafeArrayIterator(ASCII_LOWER_ALPHA),
  ];
  const ASCII_ALPHANUMERIC = [
    ...new SafeArrayIterator(ASCII_DIGIT),
    ...new SafeArrayIterator(ASCII_ALPHA),
  ];

  const HTTP_TAB_OR_SPACE = ["\u0009", "\u0020"];
  const HTTP_WHITESPACE = [
    "\u000A",
    "\u000D",
    ...new SafeArrayIterator(HTTP_TAB_OR_SPACE),
  ];

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
    ...new SafeArrayIterator(ASCII_ALPHANUMERIC),
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
  const HTTP_BETWEEN_WHITESPACE = new RegExp(
    `^[${HTTP_WHITESPACE_MATCHER}]*(.*?)[${HTTP_WHITESPACE_MATCHER}]*$`,
  );
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
    const matchers = ArrayPrototypeMap(chars, (char) => {
      if (char.length === 1) {
        const a = StringPrototypePadStart(
          NumberPrototypeToString(StringPrototypeCharCodeAt(char, 0), 16),
          4,
          "0",
        );
        return `\\u${a}`;
      } else if (char.length === 3 && char[1] === "-") {
        const a = StringPrototypePadStart(
          NumberPrototypeToString(StringPrototypeCharCodeAt(char, 0), 16),
          4,
          "0",
        );
        const b = StringPrototypePadStart(
          NumberPrototypeToString(StringPrototypeCharCodeAt(char, 2), 16),
          4,
          "0",
        );
        return `\\u${a}-\\u${b}`;
      } else {
        throw TypeError("unreachable");
      }
    });
    return ArrayPrototypeJoin(matchers, "");
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
      let c = StringPrototypeCharAt(input, position);
      position < input.length && condition(c);
      c = StringPrototypeCharAt(input, ++position)
    );
    return { result: StringPrototypeSlice(input, start, position), position };
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function byteUpperCase(s) {
    return StringPrototypeReplace(
      String(s),
      /[a-z]/g,
      function byteUpperCaseReplace(c) {
        return StringPrototypeToUpperCase(c);
      },
    );
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function byteLowerCase(s) {
    // NOTE: correct since all callers convert to ByteString first
    // TODO(@AaronO): maybe prefer a ByteString_Lower webidl converter
    return StringPrototypeToLowerCase(s);
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
    if (input[position] !== "\u0022") throw new TypeError('must be "');
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
        if (quoteOrBackslash !== "\u0022") throw new TypeError('must be "');
        // 5.6.2
        break;
      }
    }
    // 6.
    if (extractValue) return { result: value, position };
    // 7.
    return {
      result: StringPrototypeSubstring(input, positionStart, position + 1),
      position,
    };
  }

  /**
   * @param {Uint8Array} data
   * @returns {string}
   */
  function forgivingBase64Encode(data) {
    return ops.op_base64_encode(data);
  }

  /**
   * @param {string} data
   * @returns {Uint8Array}
   */
  function forgivingBase64Decode(data) {
    return ops.op_base64_decode(data);
  }

  /**
   * @param {string} char
   * @returns {boolean}
   */
  function isHttpWhitespace(char) {
    switch (char) {
      case "\u0009":
      case "\u000A":
      case "\u000D":
      case "\u0020":
        return true;
      default:
        return false;
    }
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function httpTrim(s) {
    if (!isHttpWhitespace(s[0]) && !isHttpWhitespace(s[s.length - 1])) {
      return s;
    }
    return StringPrototypeMatch(s, HTTP_BETWEEN_WHITESPACE)?.[1] ?? "";
  }

  class AssertionError extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  /**
   * @param {unknown} cond
   * @param {string=} msg
   * @returns {asserts cond}
   */
  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
    }
  }

  /**
   * @param {unknown} value
   * @returns {string}
   */
  function serializeJSValueToJSONString(value) {
    const result = JSONStringify(value);
    if (result === undefined) {
      throw new TypeError("Value is not JSON serializable.");
    }
    return result;
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
    httpTrim,
    regexMatcher,
    byteUpperCase,
    byteLowerCase,
    collectHttpQuotedString,
    forgivingBase64Encode,
    forgivingBase64Decode,
    AssertionError,
    assert,
    serializeJSValueToJSONString,
  };
})(globalThis);
