// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const {
    collectSequenceOfCodepoints,
    HTTP_WHITESPACE,
    HTTP_WHITESPACE_PREFIX_RE,
    HTTP_WHITESPACE_SUFFIX_RE,
    HTTP_QUOTED_STRING_TOKEN_POINT_RE,
    HTTP_TOKEN_CODE_POINT_RE,
    collectHttpQuotedString,
  } = window.__bootstrap.infra;

  /**
   * @typedef MimeType 
   * @property {string} type
   * @property {string} subtype
   * @property {Map<string,string>} parameters
   */

  /**
   * @param {string} input
   * @returns {MimeType | null}
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
      if (input[position] === "\u0022") {
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

  /**
   * @param {MimeType} mimeType 
   * @returns {string}
   */
  function essence(mimeType) {
    return `${mimeType.type}/${mimeType.subtype}`;
  }

  /**
   * @param {MimeType} mimeType 
   * @returns {string}
   */
  function serializeMimeType(mimeType) {
    let serialization = essence(mimeType);
    for (const param of mimeType.parameters) {
      serialization += `;${param[0]}=`;
      let value = param[1];
      if (!HTTP_TOKEN_CODE_POINT_RE.test(value)) {
        value = value.replaceAll("\\", "\\\\");
        value = value.replaceAll('"', '\\"');
        value = `"${value}"`;
      }
      serialization += value;
    }
    return serialization;
  }

  window.__bootstrap.mimesniff = { parseMimeType, essence, serializeMimeType };
})(this);
