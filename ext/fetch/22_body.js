// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../url/lib.deno_url.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../web/06_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = globalThis.__bootstrap.webidl;
  const { parseUrlEncoded } = globalThis.__bootstrap.url;
  const { parseFormData, formDataFromEntries, formDataToBlob } =
    globalThis.__bootstrap.formData;
  const mimesniff = globalThis.__bootstrap.mimesniff;
  const { isReadableStreamDisturbed, errorReadableStream, createProxy } =
    globalThis.__bootstrap.streams;
  const {
    ArrayBuffer,
    ArrayBufferIsView,
    ArrayPrototypePush,
    ArrayPrototypeMap,
    JSONParse,
    ObjectDefineProperties,
    PromiseResolve,
    TypedArrayPrototypeSet,
    TypedArrayPrototypeSlice,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

  class InnerBody {
    /** @type {ReadableStream<Uint8Array> | { body: Uint8Array, consumed: boolean }} */
    streamOrStatic;
    /** @type {null | Uint8Array | Blob | FormData} */
    source = null;
    /** @type {null | number} */
    length = null;

    /**
     * @param {ReadableStream<Uint8Array> | { body: Uint8Array, consumed: boolean }} stream
     */
    constructor(stream) {
      this.streamOrStatic = stream ??
        { body: new Uint8Array(), consumed: false };
    }

    get stream() {
      if (!(this.streamOrStatic instanceof ReadableStream)) {
        const { body, consumed } = this.streamOrStatic;
        if (consumed) {
          this.streamOrStatic = new ReadableStream();
          this.streamOrStatic.getReader();
        } else {
          this.streamOrStatic = new ReadableStream({
            start(controller) {
              controller.enqueue(body);
              controller.close();
            },
          });
        }
      }
      return this.streamOrStatic;
    }

    /**
     * https://fetch.spec.whatwg.org/#body-unusable
     * @returns {boolean}
     */
    unusable() {
      if (this.streamOrStatic instanceof ReadableStream) {
        return this.streamOrStatic.locked ||
          isReadableStreamDisturbed(this.streamOrStatic);
      }
      return this.streamOrStatic.consumed;
    }

    /**
     * @returns {boolean}
     */
    consumed() {
      if (this.streamOrStatic instanceof ReadableStream) {
        return isReadableStreamDisturbed(this.streamOrStatic);
      }
      return this.streamOrStatic.consumed;
    }

    /**
     * https://fetch.spec.whatwg.org/#concept-body-consume-body
     * @returns {Promise<Uint8Array>}
     */
    async consume() {
      if (this.unusable()) throw new TypeError("Body already consumed.");
      if (this.streamOrStatic instanceof ReadableStream) {
        const reader = this.stream.getReader();
        /** @type {Uint8Array[]} */
        const chunks = [];
        let totalLength = 0;
        while (true) {
          const { value: chunk, done } = await reader.read();
          if (done) break;
          ArrayPrototypePush(chunks, chunk);
          totalLength += chunk.byteLength;
        }
        const finalBuffer = new Uint8Array(totalLength);
        let i = 0;
        for (const chunk of chunks) {
          TypedArrayPrototypeSet(finalBuffer, chunk, i);
          i += chunk.byteLength;
        }
        return finalBuffer;
      } else {
        this.streamOrStatic.consumed = true;
        return this.streamOrStatic.body;
      }
    }

    cancel(error) {
      if (this.streamOrStatic instanceof ReadableStream) {
        this.streamOrStatic.cancel(error);
      } else {
        this.streamOrStatic.consumed = true;
      }
    }

    error(error) {
      if (this.streamOrStatic instanceof ReadableStream) {
        errorReadableStream(this.streamOrStatic, error);
      } else {
        this.streamOrStatic.consumed = true;
      }
    }

    /**
     * @returns {InnerBody}
     */
    clone() {
      const [out1, out2] = this.stream.tee();
      this.streamOrStatic = out1;
      const second = new InnerBody(out2);
      second.source = core.deserialize(core.serialize(this.source));
      second.length = this.length;
      return second;
    }

    /**
     * @returns {InnerBody}
     */
    createProxy() {
      let proxyStreamOrStatic;
      if (this.streamOrStatic instanceof ReadableStream) {
        proxyStreamOrStatic = createProxy(this.streamOrStatic);
      } else {
        proxyStreamOrStatic = { ...this.streamOrStatic };
        this.streamOrStatic.consumed = true;
      }
      const proxy = new InnerBody(proxyStreamOrStatic);
      proxy.source = this.source;
      proxy.length = this.length;
      return proxy;
    }
  }

  /**
   * @param {any} prototype
   * @param {symbol} bodySymbol
   * @param {symbol} mimeTypeSymbol
   * @returns {void}
   */
  function mixinBody(prototype, bodySymbol, mimeTypeSymbol) {
    function consumeBody(object) {
      if (object[bodySymbol] !== null) {
        return object[bodySymbol].consume();
      }
      return PromiseResolve(new Uint8Array());
    }

    /** @type {PropertyDescriptorMap} */
    const mixin = {
      body: {
        /**
         * @returns {ReadableStream<Uint8Array> | null}
         */
        get() {
          webidl.assertBranded(this, prototype);
          if (this[bodySymbol] === null) {
            return null;
          } else {
            return this[bodySymbol].stream;
          }
        },
        configurable: true,
        enumerable: true,
      },
      bodyUsed: {
        /**
         * @returns {boolean}
         */
        get() {
          webidl.assertBranded(this, prototype);
          if (this[bodySymbol] !== null) {
            return this[bodySymbol].consumed();
          }
          return false;
        },
        configurable: true,
        enumerable: true,
      },
      arrayBuffer: {
        /** @returns {Promise<ArrayBuffer>} */
        value: async function arrayBuffer() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "ArrayBuffer");
        },
        writable: true,
        configurable: true,
        enumerable: true,
      },
      blob: {
        /** @returns {Promise<Blob>} */
        value: async function blob() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "Blob", this[mimeTypeSymbol]);
        },
        writable: true,
        configurable: true,
        enumerable: true,
      },
      formData: {
        /** @returns {Promise<FormData>} */
        value: async function formData() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "FormData", this[mimeTypeSymbol]);
        },
        writable: true,
        configurable: true,
        enumerable: true,
      },
      json: {
        /** @returns {Promise<any>} */
        value: async function json() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "JSON");
        },
        writable: true,
        configurable: true,
        enumerable: true,
      },
      text: {
        /** @returns {Promise<string>} */
        value: async function text() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "text");
        },
        writable: true,
        configurable: true,
        enumerable: true,
      },
    };
    return ObjectDefineProperties(prototype.prototype, mixin);
  }

  /**
   * https://fetch.spec.whatwg.org/#concept-body-package-data
   * @param {Uint8Array} bytes
   * @param {"ArrayBuffer" | "Blob" | "FormData" | "JSON" | "text"} type
   * @param {MimeType | null} [mimeType]
   */
  function packageData(bytes, type, mimeType) {
    switch (type) {
      case "ArrayBuffer":
        return bytes.buffer;
      case "Blob":
        return new Blob([bytes], {
          type: mimeType !== null ? mimesniff.serializeMimeType(mimeType) : "",
        });
      case "FormData": {
        if (mimeType !== null) {
          const essence = mimesniff.essence(mimeType);
          if (essence === "multipart/form-data") {
            const boundary = mimeType.parameters.get("boundary");
            if (boundary === null) {
              throw new TypeError(
                "Missing boundary parameter in mime type of multipart formdata.",
              );
            }
            return parseFormData(bytes, boundary);
          } else if (essence === "application/x-www-form-urlencoded") {
            const entries = parseUrlEncoded(bytes);
            return formDataFromEntries(
              ArrayPrototypeMap(
                entries,
                (x) => ({ name: x[0], value: x[1] }),
              ),
            );
          }
          throw new TypeError("Body can not be decoded as form data");
        }
        throw new TypeError("Missing content type");
      }
      case "JSON":
        return JSONParse(core.decode(bytes));
      case "text":
        return core.decode(bytes);
    }
  }

  /**
   * @param {BodyInit} object
   * @returns {{body: InnerBody, contentType: string | null}}
   */
  function extractBody(object) {
    /** @type {ReadableStream<Uint8Array> | { body: Uint8Array, consumed: boolean }} */
    let stream;
    let source = null;
    let length = null;
    let contentType = null;
    if (object instanceof Blob) {
      stream = object.stream();
      source = object;
      length = object.size;
      if (object.type.length !== 0) {
        contentType = object.type;
      }
    } else if (ArrayBufferIsView(object) || object instanceof ArrayBuffer) {
      const u8 = ArrayBufferIsView(object)
        ? new Uint8Array(
          object.buffer,
          object.byteOffset,
          object.byteLength,
        )
        : new Uint8Array(object);
      const copy = TypedArrayPrototypeSlice(u8, 0, u8.byteLength);
      source = copy;
    } else if (object instanceof FormData) {
      const res = formDataToBlob(object);
      stream = res.stream();
      source = res;
      length = res.size;
      contentType = res.type;
    } else if (object instanceof URLSearchParams) {
      // TODO(@satyarohith): not sure what primordial here.
      source = core.encode(object.toString());
      contentType = "application/x-www-form-urlencoded;charset=UTF-8";
    } else if (typeof object === "string") {
      source = core.encode(object);
      contentType = "text/plain;charset=UTF-8";
    } else if (object instanceof ReadableStream) {
      stream = object;
      if (object.locked || isReadableStreamDisturbed(object)) {
        throw new TypeError("ReadableStream is locked or disturbed");
      }
    }
    if (source instanceof Uint8Array) {
      stream = { body: source, consumed: false };
      length = source.byteLength;
    }
    const body = new InnerBody(stream);
    body.source = source;
    body.length = length;
    return { body, contentType };
  }

  webidl.converters["BodyInit"] = (V, opts) => {
    // Union for (ReadableStream or Blob or ArrayBufferView or ArrayBuffer or FormData or URLSearchParams or USVString)
    if (V instanceof ReadableStream) {
      // TODO(lucacasonato): ReadableStream is not branded
      return V;
    } else if (V instanceof Blob) {
      return webidl.converters["Blob"](V, opts);
    } else if (V instanceof FormData) {
      return webidl.converters["FormData"](V, opts);
    } else if (V instanceof URLSearchParams) {
      // TODO(lucacasonato): URLSearchParams is not branded
      return V;
    }
    if (typeof V === "object") {
      if (V instanceof ArrayBuffer || V instanceof SharedArrayBuffer) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBufferIsView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };
  webidl.converters["BodyInit?"] = webidl.createNullableConverter(
    webidl.converters["BodyInit"],
  );

  window.__bootstrap.fetchBody = { mixinBody, InnerBody, extractBody };
})(globalThis);
