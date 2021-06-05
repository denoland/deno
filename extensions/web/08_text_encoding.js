// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference lib="esnext" />

"use strict";

((window) => {
  const core = Deno.core;
  const webidl = window.__bootstrap.webidl;

  class TextDecoder {
    /** @type {string} */
    #encoding;
    /** @type {boolean} */
    #fatal;
    /** @type {boolean} */
    #ignoreBOM;

    /** @type {number | null} */
    #rid = null;

    /**
     *
     * @param {string} label
     * @param {TextDecoderOptions} options
     */
    constructor(label = "utf-8", options = {}) {
      const prefix = "Failed to construct 'TextDecoder'";
      label = webidl.converters.DOMString(label, {
        prefix,
        context: "Argument 1",
      });
      options = webidl.converters.TextDecoderOptions(options, {
        prefix,
        context: "Argument 2",
      });
      const encoding = core.opSync("op_encoding_normalize_label", label);
      this.#encoding = encoding;
      this.#fatal = options.fatal;
      this.#ignoreBOM = options.ignoreBOM;
      this[webidl.brand] = webidl.brand;
    }

    /** @returns {string} */
    get encoding() {
      webidl.assertBranded(this, TextDecoder);
      return this.#encoding;
    }

    /** @returns {boolean} */
    get fatal() {
      webidl.assertBranded(this, TextDecoder);
      return this.#fatal;
    }

    /** @returns {boolean} */
    get ignoreBOM() {
      webidl.assertBranded(this, TextDecoder);
      return this.#ignoreBOM;
    }

    /**
     * @param {BufferSource} [input]
     * @param {TextDecodeOptions} options
     */
    decode(input = new Uint8Array(), options = {}) {
      webidl.assertBranded(this, TextDecoder);
      const prefix = "Failed to execute 'decode' on 'TextDecoder'";
      if (input !== undefined) {
        input = webidl.converters.BufferSource(input, {
          prefix,
          context: "Argument 1",
          allowShared: true,
        });
      }
      options = webidl.converters.TextDecodeOptions(options, {
        prefix,
        context: "Argument 2",
      });

      // TODO(lucacasonato): add fast path for non-streaming decoder & decode

      if (this.#rid === null) {
        this.#rid = core.opSync("op_encoding_new_decoder", {
          label: this.#encoding,
          fatal: this.#fatal,
          ignoreBom: this.#ignoreBOM,
        });
      }

      try {
        if (ArrayBuffer.isView(input)) {
          input = new Uint8Array(
            input.buffer,
            input.byteOffset,
            input.byteLength,
          );
        } else {
          input = new Uint8Array(input);
        }
        return core.opSync("op_encoding_decode", new Uint8Array(input), {
          rid: this.#rid,
          stream: options.stream,
        });
      } finally {
        if (!options.stream) {
          core.close(this.#rid);
          this.#rid = null;
        }
      }
    }

    get [Symbol.toStringTag]() {
      return "TextDecoder";
    }
  }

  Object.defineProperty(TextDecoder.prototype, "encoding", {
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(TextDecoder.prototype, "fatal", {
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(TextDecoder.prototype, "ignoreBOM", {
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(TextDecoder.prototype, "decode", {
    enumerable: true,
    writable: true,
    configurable: true,
  });

  class TextEncoder {
    constructor() {
      this[webidl.brand] = webidl.brand;
    }

    /** @returns {string} */
    get encoding() {
      webidl.assertBranded(this, TextEncoder);
      return "utf-8";
    }

    /**
     * @param {string} input
     * @returns {Uint8Array}
     */
    encode(input = "") {
      webidl.assertBranded(this, TextEncoder);
      const prefix = "Failed to execute 'encode' on 'TextEncoder'";
      input = webidl.converters.USVString(input, {
        prefix,
        context: "Argument 1",
      });
      return core.encode(input);
    }

    /**
     * @param {string} source
     * @param {Uint8Array} destination
     * @returns {TextEncoderEncodeIntoResult}
     */
    encodeInto(source, destination) {
      webidl.assertBranded(this, TextEncoder);
      const prefix = "Failed to execute 'encodeInto' on 'TextEncoder'";
      source = webidl.converters.USVString(source, {
        prefix,
        context: "Argument 1",
      });
      destination = webidl.converters.Uint8Array(destination, {
        prefix,
        context: "Argument 2",
        allowShared: true,
      });
      return core.opSync("op_encoding_encode_into", source, destination);
    }

    get [Symbol.toStringTag]() {
      return "TextEncoder";
    }
  }

  Object.defineProperty(TextEncoder.prototype, "encoding", {
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(TextEncoder.prototype, "encode", {
    enumerable: true,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(TextEncoder.prototype, "encodeInto", {
    enumerable: true,
    writable: true,
    configurable: true,
  });

  webidl.converters.TextDecoderOptions = webidl.createDictionaryConverter(
    "TextDecoderOptions",
    [
      {
        key: "fatal",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
      {
        key: "ignoreBOM",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
    ],
  );
  webidl.converters.TextDecodeOptions = webidl.createDictionaryConverter(
    "TextDecodeOptions",
    [
      {
        key: "stream",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
    ],
  );

  /**
   * @param {Uint8Array} bytes
   */
  function decode(bytes, encoding) {
    const BOMEncoding = BOMSniff(bytes);
    let start = 0;
    if (BOMEncoding !== null) {
      encoding = BOMEncoding;
      if (BOMEncoding === "UTF-8") start = 3;
      else start = 2;
    }
    return new TextDecoder(encoding).decode(bytes.slice(start));
  }

  /**
   * @param {Uint8Array} bytes
   */
  function BOMSniff(bytes) {
    const BOM = bytes.subarray(0, 3);
    if (BOM[0] === 0xEF && BOM[1] === 0xBB && BOM[2] === 0xBF) {
      return "UTF-8";
    }
    if (BOM[0] === 0xFE && BOM[1] === 0xFF) return "UTF-16BE";
    if (BOM[0] === 0xFF && BOM[1] === 0xFE) return "UTF-16LE";
    return null;
  }

  window.__bootstrap.encoding = {
    TextEncoder,
    TextDecoder,
    decode,
  };
})(this);
