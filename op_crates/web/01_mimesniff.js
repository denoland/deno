// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const { collectSequenceOfCodepoints } = window.__bootstrap.infra;

  /**
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

  const HTTP_TAB_OR_SPACE = ["\u0009", "\u0020"];
  const HTTP_WHITESPACE = ["\u000A", "\u000D", ...HTTP_TAB_OR_SPACE];

  const ASCII_DIGIT = ["\u0030-\u0039"];
  const ASCII_UPPER_ALPHA = ["\u0041-\u005A"];
  const ASCII_LOWER_ALPHA = ["\u0061-\u007A"];
  const ASCII_ALPHA = [...ASCII_UPPER_ALPHA, ...ASCII_LOWER_ALPHA];
  const ASCII_ALPHANUMERIC = [...ASCII_DIGIT, ...ASCII_ALPHA];
  const HTTP_TOKEN_CODE_POINT = [
    "\u0021",
    "\u0023",
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
        if (input[position] !== "\u0022") throw new Error('must be "');
        // 5.6.2
        break;
      }
    }
    // 6.
    if (extractValue) return { result: value, position };
    // 7.
    return { result: input.substring(positionStart, position + 1), position };
  }

  /**
   * @param {string} input
   */
  function parseMimeType(input) {
    // 1.
    input = input.replaceAll(HTTP_WHITESPACE_PREFIX_RE, "");
    input = input.replaceAll(HTTP_WHITESPACE_SUFFIX_RE, "");

    // 2.
    let position = 0;
    const endOfInput = input.length;

    // 3.
    const res1 = collectSequenceOfCodepoints(
      input,
      position,
      (c) => c != "\u002F",
    );
    const type = res1.result;
    position = res1.position;

    // 4.
    if (type === "" || !HTTP_TOKEN_CODE_POINT_RE.test(type)) return null;

    // 5.
    if (position >= endOfInput) return null;

    // 6.
    position++;

    // 7.
    const res2 = collectSequenceOfCodepoints(
      input,
      position,
      (c) => c != "\u003B",
    );
    let subtype = res2.result;
    position = res2.position;

    // 8.
    subtype = subtype.replaceAll(HTTP_WHITESPACE_SUFFIX_RE, "");

    // 9.
    if (subtype === "" || !HTTP_TOKEN_CODE_POINT_RE.test(subtype)) return null;

    // 10.
    const mimeType = {
      type: type.toLowerCase(),
      subtype: subtype.toLowerCase(),
      /** @type {Map<string, string>} */
      parameters: new Map(),
    };

    // 11.
    while (position < endOfInput) {
      // 11.1.
      position++;

      // 11.2.
      const res1 = collectSequenceOfCodepoints(
        input,
        position,
        (c) => HTTP_WHITESPACE.includes(c),
      );
      position = res1.position;

      // 11.3.
      const res2 = collectSequenceOfCodepoints(
        input,
        position,
        (c) => c !== "\u003B" && c !== "\u003D",
      );
      let parameterName = res2.result;
      position = res2.position;

      // 11.4.
      parameterName = parameterName.toLowerCase();

      // 11.5.
      if (position < endOfInput) {
        if (input[position] == "\u003B") continue;
        position++;
      }

      // 11.6.
      if (position >= endOfInput) break;

      // 11.7.
      let parameterValue = null;

      // 11.8.
      if (input[position] == "\u0022") {
        // 11.8.1.
        const res = collectHttpQuotedString(input, position, true);
        parameterValue = res.result;
        position = res.position;

        // 11.8.2.
        position++;
      } else { // 11.9.
        // 11.9.1.
        const res = collectSequenceOfCodepoints(
          input,
          position,
          (c) => c !== "\u003B",
        );
        parameterValue = res.result;
        position = res.position;

        // 11.9.2.
        parameterValue = parameterValue.replaceAll(
          HTTP_WHITESPACE_SUFFIX_RE,
          "",
        );

        // 11.9.3.
        if (parameterValue === "") continue;
      }

      // 11.10.
      if (
        parameterName !== "" && HTTP_TOKEN_CODE_POINT_RE.test(parameterName) &&
        HTTP_QUOTED_STRING_TOKEN_POINT_RE.test(parameterValue) &&
        !mimeType.parameters.has(parameterName)
      ) {
        mimeType.parameters.set(parameterName, parameterValue);
      }
    }

    // 12.
    return mimeType;
  }

  window.__bootstrap.mimesniff = { parseMimeType };
})(this);
