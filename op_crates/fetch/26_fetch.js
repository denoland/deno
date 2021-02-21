// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;

  // provided by "deno_web"
  const { URLSearchParams } = window.__bootstrap.url;
  const { getLocationHref } = window.__bootstrap.location;

  const {
    createDictionaryConverter,
    createEnumConverter,
    createNullableConverter,
    converters,
  } = window.__bootstrap.webidl;

  const { requiredArguments } = window.__bootstrap.fetchUtil;
  const { ReadableStream, isReadableStreamDisturbed } =
    window.__bootstrap.streams;
  const { DomIterableMixin } = window.__bootstrap.domIterable;
  const { Headers } = window.__bootstrap.headers;
  const { Blob, _byteSequence, File } = window.__bootstrap.file;

  const MAX_SIZE = 2 ** 32 - 2;

  /**
   * @param {Uint8Array} src 
   * @param {Uint8Array} dst 
   * @param {number} off the offset into `dst` where it will at which to begin writing values from `src`
   * 
   * @returns {number} number of bytes copied
   */
  function copyBytes(src, dst, off = 0) {
    const r = dst.byteLength - off;
    if (src.byteLength > r) {
      src = src.subarray(0, r);
    }
    dst.set(src, off);
    return src.byteLength;
  }

  class Buffer {
    /** @type {Uint8Array} */
    #buf; // contents are the bytes buf[off : len(buf)]
    #off = 0; // read at buf[off], write at buf[buf.byteLength]

    /** @param {ArrayBuffer} [ab] */
    constructor(ab) {
      if (ab == null) {
        this.#buf = new Uint8Array(0);
        return;
      }

      this.#buf = new Uint8Array(ab);
    }

    /**
     * @returns {Uint8Array}
     */
    bytes(options = { copy: true }) {
      if (options.copy === false) return this.#buf.subarray(this.#off);
      return this.#buf.slice(this.#off);
    }

    /**
     * @returns {boolean}
     */
    empty() {
      return this.#buf.byteLength <= this.#off;
    }

    /**
     * @returns {number}
     */
    get length() {
      return this.#buf.byteLength - this.#off;
    }

    /**
     * @returns {number}
     */
    get capacity() {
      return this.#buf.buffer.byteLength;
    }

    /**
     * @returns {void}
     */
    reset() {
      this.#reslice(0);
      this.#off = 0;
    }

    /**
     * @param {number} n
     * @returns {number}
     */
    #tryGrowByReslice = (n) => {
      const l = this.#buf.byteLength;
      if (n <= this.capacity - l) {
        this.#reslice(l + n);
        return l;
      }
      return -1;
    };

    /**
     * @param {number} len
     * @returns {void}
     */
    #reslice = (len) => {
      if (!(len <= this.#buf.buffer.byteLength)) {
        throw new Error("assert");
      }
      this.#buf = new Uint8Array(this.#buf.buffer, 0, len);
    };

    /**
     * @param {Uint8Array} p
     * @returns {number}
     */
    writeSync(p) {
      const m = this.#grow(p.byteLength);
      return copyBytes(p, this.#buf, m);
    }

    /**
     * @param {Uint8Array} p
     * @returns {Promise<number>}
     */
    write(p) {
      const n = this.writeSync(p);
      return Promise.resolve(n);
    }

    /** 
     * @param {number} n
     * @returns {number}
     */
    #grow = (n) => {
      const m = this.length;
      // If buffer is empty, reset to recover space.
      if (m === 0 && this.#off !== 0) {
        this.reset();
      }
      // Fast: Try to grow by means of a reslice.
      const i = this.#tryGrowByReslice(n);
      if (i >= 0) {
        return i;
      }
      const c = this.capacity;
      if (n <= Math.floor(c / 2) - m) {
        // We can slide things down instead of allocating a new
        // ArrayBuffer. We only need m+n <= c to slide, but
        // we instead let capacity get twice as large so we
        // don't spend all our time copying.
        copyBytes(this.#buf.subarray(this.#off), this.#buf);
      } else if (c + n > MAX_SIZE) {
        throw new Error("The buffer cannot be grown beyond the maximum size.");
      } else {
        // Not enough space anywhere, we need to allocate.
        const buf = new Uint8Array(Math.min(2 * c + n, MAX_SIZE));
        copyBytes(this.#buf.subarray(this.#off), buf);
        this.#buf = buf;
      }
      // Restore this.#off and len(this.#buf).
      this.#off = 0;
      this.#reslice(Math.min(m + n, MAX_SIZE));
      return m;
    };

    /** 
     * @param {number} n
     * @returns {void}
     */
    grow(n) {
      if (n < 0) {
        throw Error("Buffer.grow: negative count");
      }
      const m = this.#grow(n);
      this.#reslice(m);
    }
  }

  const TypedArray = Reflect.getPrototypeOf(Int8Array);
  /** 
   * @param {unknown} x
   * @returns {x is ArrayBufferView}
   */
  function isTypedArray(x) {
    return x instanceof TypedArray;
  }

  /** 
   * @param {string} s
   * @param {string} value
   * @returns {boolean}
   */
  function hasHeaderValueOf(s, value) {
    return new RegExp(`^${value}(?:[\\s;]|$)`).test(s);
  }

  /**
   * @param {string} value
   * @returns {Map<string, string>}
   */
  function getHeaderValueParams(value) {
    /** @type {Map<string, string>} */
    const params = new Map();
    // Forced to do so for some Map constructor param mismatch
    value
      .split(";")
      .slice(1)
      .map((s) => s.trim().split("="))
      .filter((arr) => arr.length > 1)
      .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
      .forEach(([k, v]) => params.set(k, v));
    return params;
  }

  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  const CR = "\r".charCodeAt(0);
  const LF = "\n".charCodeAt(0);

  const dataSymbol = Symbol("data");

  /**
   * @param {Blob | string} value 
   * @param {string | undefined} filename
   * @returns {FormDataEntryValue}
   */
  function parseFormDataValue(value, filename) {
    if (value instanceof File) {
      return new File([value], filename || value.name, {
        type: value.type,
        lastModified: value.lastModified,
      });
    } else if (value instanceof Blob) {
      return new File([value], filename || "blob", {
        type: value.type,
      });
    } else {
      return String(value);
    }
  }

  class FormDataBase {
    /** @type {[name: string, entry: FormDataEntryValue][]} */
    [dataSymbol] = [];

    /**
     * @param {string} name 
     * @param {string | Blob} value 
     * @param {string} [filename] 
     * @returns {void}
     */
    append(name, value, filename) {
      requiredArguments("FormData.append", arguments.length, 2);
      name = String(name);
      this[dataSymbol].push([name, parseFormDataValue(value, filename)]);
    }

    /**
     * @param {string} name 
     * @returns {void}
     */
    delete(name) {
      requiredArguments("FormData.delete", arguments.length, 1);
      name = String(name);
      let i = 0;
      while (i < this[dataSymbol].length) {
        if (this[dataSymbol][i][0] === name) {
          this[dataSymbol].splice(i, 1);
        } else {
          i++;
        }
      }
    }

    /**
     * @param {string} name 
     * @returns {FormDataEntryValue[]}
     */
    getAll(name) {
      requiredArguments("FormData.getAll", arguments.length, 1);
      name = String(name);
      const values = [];
      for (const entry of this[dataSymbol]) {
        if (entry[0] === name) {
          values.push(entry[1]);
        }
      }

      return values;
    }

    /**
     * @param {string} name 
     * @returns {FormDataEntryValue | null}
     */
    get(name) {
      requiredArguments("FormData.get", arguments.length, 1);
      name = String(name);
      for (const entry of this[dataSymbol]) {
        if (entry[0] === name) {
          return entry[1];
        }
      }

      return null;
    }

    /**
     * @param {string} name 
     * @returns {boolean}
     */
    has(name) {
      requiredArguments("FormData.has", arguments.length, 1);
      name = String(name);
      return this[dataSymbol].some((entry) => entry[0] === name);
    }

    /**
     * @param {string} name 
     * @param {string | Blob} value 
     * @param {string} [filename] 
     * @returns {void}
     */
    set(name, value, filename) {
      requiredArguments("FormData.set", arguments.length, 2);
      name = String(name);

      // If there are any entries in the context object’s entry list whose name
      // is name, replace the first such entry with entry and remove the others
      let found = false;
      let i = 0;
      while (i < this[dataSymbol].length) {
        if (this[dataSymbol][i][0] === name) {
          if (!found) {
            this[dataSymbol][i][1] = parseFormDataValue(value, filename);
            found = true;
          } else {
            this[dataSymbol].splice(i, 1);
            continue;
          }
        }
        i++;
      }

      // Otherwise, append entry to the context object’s entry list.
      if (!found) {
        this[dataSymbol].push([name, parseFormDataValue(value, filename)]);
      }
    }

    get [Symbol.toStringTag]() {
      return "FormData";
    }
  }

  class FormData extends DomIterableMixin(FormDataBase, dataSymbol) {}

  class MultipartBuilder {
    /**
     * @param {FormData} formData 
     * @param {string} [boundary] 
     */
    constructor(formData, boundary) {
      this.formData = formData;
      this.boundary = boundary ?? this.#createBoundary();
      this.writer = new Buffer();
    }

    /** 
     * @returns {string}
     */
    getContentType() {
      return `multipart/form-data; boundary=${this.boundary}`;
    }

    /** 
     * @returns {Uint8Array}
     */
    getBody() {
      for (const [fieldName, fieldValue] of this.formData.entries()) {
        if (fieldValue instanceof File) {
          this.#writeFile(fieldName, fieldValue);
        } else this.#writeField(fieldName, fieldValue);
      }

      this.writer.writeSync(encoder.encode(`\r\n--${this.boundary}--`));

      return this.writer.bytes();
    }

    #createBoundary = () => {
      return (
        "----------" +
        Array.from(Array(32))
          .map(() => Math.random().toString(36)[2] || 0)
          .join("")
      );
    };

    /** 
     * @param {[string, string][]} headers
     * @returns {void}
     */
    #writeHeaders = (headers) => {
      let buf = this.writer.empty() ? "" : "\r\n";

      buf += `--${this.boundary}\r\n`;
      for (const [key, value] of headers) {
        buf += `${key}: ${value}\r\n`;
      }
      buf += `\r\n`;

      this.writer.writeSync(encoder.encode(buf));
    };

    /** 
     * @param {string} field
     * @param {string} filename
     * @param {string} [type]
     * @returns {void}
     */
    #writeFileHeaders = (
      field,
      filename,
      type,
    ) => {
      /** @type {[string, string][]} */
      const headers = [
        [
          "Content-Disposition",
          `form-data; name="${field}"; filename="${filename}"`,
        ],
        ["Content-Type", type || "application/octet-stream"],
      ];
      return this.#writeHeaders(headers);
    };

    /**
     * @param {string} field
     * @returns {void}
     */
    #writeFieldHeaders = (field) => {
      /** @type {[string, string][]} */
      const headers = [["Content-Disposition", `form-data; name="${field}"`]];
      return this.#writeHeaders(headers);
    };

    /**
     * @param {string} field
     * @param {string} value
     * @returns {void}
     */
    #writeField = (field, value) => {
      this.#writeFieldHeaders(field);
      this.writer.writeSync(encoder.encode(value));
    };

    /**
     * @param {string} field
     * @param {File} value
     * @returns {void}
     */
    #writeFile = (field, value) => {
      this.#writeFileHeaders(field, value.name, value.type);
      this.writer.writeSync(value[_byteSequence]);
    };
  }

  class MultipartParser {
    /**
     * @param {Uint8Array} body 
     * @param {string | undefined} boundary 
     */
    constructor(body, boundary) {
      if (!boundary) {
        throw new TypeError("multipart/form-data must provide a boundary");
      }

      this.boundary = `--${boundary}`;
      this.body = body;
      this.boundaryChars = encoder.encode(this.boundary);
    }

    /**
     * @param {string} headersText
     * @returns {{ headers: Headers, disposition: Map<string, string> }}
     */
    #parseHeaders = (headersText) => {
      const headers = new Headers();
      const rawHeaders = headersText.split("\r\n");
      for (const rawHeader of rawHeaders) {
        const sepIndex = rawHeader.indexOf(":");
        if (sepIndex < 0) {
          continue; // Skip this header
        }
        const key = rawHeader.slice(0, sepIndex);
        const value = rawHeader.slice(sepIndex + 1);
        headers.set(key, value);
      }

      return {
        headers,
        disposition: getHeaderValueParams(
          headers.get("Content-Disposition") ?? "",
        ),
      };
    };

    /**
     * @returns {FormData}
     */
    parse() {
      const formData = new FormData();
      let headerText = "";
      let boundaryIndex = 0;
      let state = 0;
      let fileStart = 0;

      for (let i = 0; i < this.body.length; i++) {
        const byte = this.body[i];
        const prevByte = this.body[i - 1];
        const isNewLine = byte === LF && prevByte === CR;

        if (state === 1 || state === 2 || state == 3) {
          headerText += String.fromCharCode(byte);
        }
        if (state === 0 && isNewLine) {
          state = 1;
        } else if (state === 1 && isNewLine) {
          state = 2;
          const headersDone = this.body[i + 1] === CR &&
            this.body[i + 2] === LF;

          if (headersDone) {
            state = 3;
          }
        } else if (state === 2 && isNewLine) {
          state = 3;
        } else if (state === 3 && isNewLine) {
          state = 4;
          fileStart = i + 1;
        } else if (state === 4) {
          if (this.boundaryChars[boundaryIndex] !== byte) {
            boundaryIndex = 0;
          } else {
            boundaryIndex++;
          }

          if (boundaryIndex >= this.boundary.length) {
            const { headers, disposition } = this.#parseHeaders(headerText);
            const content = this.body.subarray(
              fileStart,
              i - boundaryIndex - 1,
            );
            // https://fetch.spec.whatwg.org/#ref-for-dom-body-formdata
            const filename = disposition.get("filename");
            const name = disposition.get("name");

            state = 5;
            // Reset
            boundaryIndex = 0;
            headerText = "";

            if (!name) {
              continue; // Skip, unknown name
            }

            if (filename) {
              const blob = new Blob([content], {
                type: headers.get("Content-Type") || "application/octet-stream",
              });
              formData.append(name, blob, filename);
            } else {
              formData.append(name, decoder.decode(content));
            }
          }
        } else if (state === 5 && isNewLine) {
          state = 1;
        }
      }

      return formData;
    }
  }

  /**
   * @param {string} name 
   * @param {BodyInit | null} bodySource 
   */
  function validateBodyType(name, bodySource) {
    if (isTypedArray(bodySource)) {
      return true;
    } else if (bodySource instanceof ArrayBuffer) {
      return true;
    } else if (typeof bodySource === "string") {
      return true;
    } else if (bodySource instanceof ReadableStream) {
      return true;
    } else if (bodySource instanceof FormData) {
      return true;
    } else if (bodySource instanceof URLSearchParams) {
      return true;
    } else if (!bodySource) {
      return true; // null body is fine
    }
    throw new TypeError(
      `Bad ${name} body type: ${bodySource.constructor.name}`,
    );
  }

  /**
   * @param {ReadableStreamReader<Uint8Array>} stream 
   * @param {number} [size] 
   */
  async function bufferFromStream(
    stream,
    size,
  ) {
    const encoder = new TextEncoder();
    const buffer = new Buffer();

    if (size) {
      // grow to avoid unnecessary allocations & copies
      buffer.grow(size);
    }

    while (true) {
      const { done, value } = await stream.read();

      if (done) break;

      if (typeof value === "string") {
        buffer.writeSync(encoder.encode(value));
      } else if (value instanceof ArrayBuffer) {
        buffer.writeSync(new Uint8Array(value));
      } else if (value instanceof Uint8Array) {
        buffer.writeSync(value);
      } else if (!value) {
        // noop for undefined
      } else {
        throw new Error("unhandled type on stream read");
      }
    }

    return buffer.bytes().buffer;
  }

  /**
   * @param {Exclude<BodyInit, ReadableStream> | null} bodySource 
   */
  function bodyToArrayBuffer(bodySource) {
    if (isTypedArray(bodySource)) {
      return bodySource.buffer;
    } else if (bodySource instanceof ArrayBuffer) {
      return bodySource;
    } else if (typeof bodySource === "string") {
      const enc = new TextEncoder();
      return enc.encode(bodySource).buffer;
    } else if (
      bodySource instanceof FormData ||
      bodySource instanceof URLSearchParams
    ) {
      const enc = new TextEncoder();
      return enc.encode(bodySource.toString()).buffer;
    } else if (!bodySource) {
      return new ArrayBuffer(0);
    }
    throw new Error(
      `Body type not implemented: ${bodySource.constructor.name}`,
    );
  }

  const BodyUsedError =
    "Failed to execute 'clone' on 'Body': body is already used";

  const teeBody = Symbol("Body#tee");

  // fastBody and dontValidateUrl allow users to opt out of certain behaviors
  const fastBody = Symbol("Body#fast");
  const dontValidateUrl = Symbol("dontValidateUrl");

  class Body {
    #contentType = "";
    #size;
    /** @type {BodyInit | null} */
    #bodySource;
    /** @type {ReadableStream<Uint8Array> | null} */
    #stream = null;

    /**
     * @param {BodyInit| null} bodySource 
     * @param {{contentType: string, size?: number}} meta 
     */
    constructor(bodySource, meta) {
      validateBodyType(this.constructor.name, bodySource);
      this.#bodySource = bodySource;
      this.#contentType = meta.contentType;
      this.#size = meta.size;
    }

    get body() {
      if (!this.#stream) {
        if (!this.#bodySource) {
          return null;
        } else if (this.#bodySource instanceof ReadableStream) {
          this.#stream = this.#bodySource;
        } else {
          const buf = bodyToArrayBuffer(this.#bodySource);
          if (!(buf instanceof ArrayBuffer)) {
            throw new Error(
              `Expected ArrayBuffer from body`,
            );
          }

          this.#stream = new ReadableStream({
            /**
             * @param {ReadableStreamDefaultController<Uint8Array>} controller 
             */
            start(controller) {
              controller.enqueue(new Uint8Array(buf));
              controller.close();
            },
          });
        }
      }

      return this.#stream;
    }

    // Optimization that allows caller to bypass expensive ReadableStream.
    [fastBody]() {
      if (!this.#bodySource) {
        return null;
      } else if (!(this.#bodySource instanceof ReadableStream)) {
        return bodyToArrayBuffer(this.#bodySource);
      } else {
        return this.body;
      }
    }

    /** @returns {BodyInit | null} */
    [teeBody]() {
      if (this.#stream || this.#bodySource instanceof ReadableStream) {
        const body = this.body;
        if (body) {
          const [stream1, stream2] = body.tee();
          this.#stream = stream1;
          return stream2;
        } else {
          return null;
        }
      }

      return this.#bodySource;
    }

    get bodyUsed() {
      if (this.body && isReadableStreamDisturbed(this.body)) {
        return true;
      }
      return false;
    }

    set bodyUsed(_) {
      // this is a noop per spec
    }

    /** @returns {Promise<Blob>} */
    async blob() {
      return new Blob([await this.arrayBuffer()], {
        type: this.#contentType,
      });
    }

    // ref: https://fetch.spec.whatwg.org/#body-mixin
    /** @returns {Promise<FormData>} */
    async formData() {
      const formData = new FormData();
      if (hasHeaderValueOf(this.#contentType, "multipart/form-data")) {
        const params = getHeaderValueParams(this.#contentType);

        // ref: https://tools.ietf.org/html/rfc2046#section-5.1
        const boundary = params.get("boundary");
        const body = new Uint8Array(await this.arrayBuffer());
        const multipartParser = new MultipartParser(body, boundary);

        return multipartParser.parse();
      } else if (
        hasHeaderValueOf(this.#contentType, "application/x-www-form-urlencoded")
      ) {
        // From https://github.com/github/fetch/blob/master/fetch.js
        // Copyright (c) 2014-2016 GitHub, Inc. MIT License
        const body = await this.text();
        try {
          body
            .trim()
            .split("&")
            .forEach((bytes) => {
              if (bytes) {
                const split = bytes.split("=");
                if (split.length >= 2) {
                  // @ts-expect-error this is safe because of the above check
                  const name = split.shift().replace(/\+/g, " ");
                  const value = split.join("=").replace(/\+/g, " ");
                  formData.append(
                    decodeURIComponent(name),
                    decodeURIComponent(value),
                  );
                }
              }
            });
        } catch (e) {
          throw new TypeError("Invalid form urlencoded format");
        }
        return formData;
      } else {
        throw new TypeError("Invalid form data");
      }
    }

    /** @returns {Promise<string>} */
    async text() {
      if (typeof this.#bodySource === "string") {
        return this.#bodySource;
      }

      const ab = await this.arrayBuffer();
      const decoder = new TextDecoder("utf-8");
      return decoder.decode(ab);
    }

    /** @returns {Promise<any>} */
    async json() {
      const raw = await this.text();
      return JSON.parse(raw);
    }

    /** @returns {Promise<ArrayBuffer>} */
    arrayBuffer() {
      if (this.#bodySource instanceof ReadableStream) {
        const body = this.body;
        if (!body) throw new TypeError("Unreachable state (no body)");
        return bufferFromStream(body.getReader(), this.#size);
      }
      return Promise.resolve(bodyToArrayBuffer(this.#bodySource));
    }
  }

  /**
   * @param {Deno.CreateHttpClientOptions} options
   * @returns {HttpClient}
   */
  function createHttpClient(options) {
    return new HttpClient(core.jsonOpSync("op_create_http_client", options));
  }

  class HttpClient {
    /**
     * @param {number} rid 
     */
    constructor(rid) {
      this.rid = rid;
    }
    close() {
      core.close(this.rid);
    }
  }

  /**
   * @param {{ headers: [string,string][], method: string, url: string, baseUrl: string | null, clientRid: number | null, hasBody: boolean }} args 
   * @param {Uint8Array | null} body 
   * @returns {{requestRid: number, requestBodyRid: number | null}}
   */
  function opFetch(args, body) {
    let zeroCopy;
    if (body != null) {
      zeroCopy = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
    }
    return core.jsonOpSync("op_fetch", args, ...(zeroCopy ? [zeroCopy] : []));
  }

  /**
   * @param {{rid: number}} args
   * @returns {Promise<{status: number, statusText: string, headers: Record<string,string[]>, url: string, responseRid: number}>}
   */
  function opFetchSend(args) {
    return core.jsonOpAsync("op_fetch_send", args);
  }

  /**
   * @param {{rid: number}} args 
   * @param {Uint8Array} body 
   * @returns {Promise<void>}
   */
  function opFetchRequestWrite(args, body) {
    const zeroCopy = new Uint8Array(
      body.buffer,
      body.byteOffset,
      body.byteLength,
    );
    return core.jsonOpAsync("op_fetch_request_write", args, zeroCopy);
  }

  const NULL_BODY_STATUS = [101, 204, 205, 304];
  const REDIRECT_STATUS = [301, 302, 303, 307, 308];

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
   * @param {string} m
   * @returns {string}
   */
  function normalizeMethod(m) {
    const u = byteUpperCase(m);
    if (
      u === "DELETE" ||
      u === "GET" ||
      u === "HEAD" ||
      u === "OPTIONS" ||
      u === "POST" ||
      u === "PUT"
    ) {
      return u;
    }
    return m;
  }

  // "referrer", "referrerPolicy", "mode", "cache", "signal", and "cancelable" are all un-used in current implementation; the converters are dead code until then

  const todo = () => {
    throw new Error("todo!");
  };

  // https://fetch.spec.whatwg.org/#requestinfo
  // typedef (Request or USVString) RequestInfo;
  // const requestInfoConverter = webidl.union([ Request, USVString ]);
  const requestInfoConverter = todo;

  // Headers should be able to convert values to HeadersInit,
  // internally work with Headers directly
  const headersInitConverter = (v) => new Headers(v);

  // https://fetch.spec.whatwg.org/#bodyinit
  const bodyInitConverter = todo;

  // https://w3c.github.io/webappsec-referrer-policy/#enumdef-referrerpolicy
  const referrerPolicyConverter = todo;

  // https://fetch.spec.whatwg.org/#requestmode
  const requestModeConverter = createEnumConverter(
    "RequestMode",
    ["navigate", "same-origin", "no-cors", "cors"],
  );

  // https://fetch.spec.whatwg.org/#requestcredentials
  const requestCredentialsConverter = createEnumConverter(
    "RequestCredentials",
    ["omit", "same-origin", "include"],
  );

  // https://fetch.spec.whatwg.org/#requestcache
  const requestCacheConverter = createEnumConverter(
    "RequestCache",
    [
      "default",
      "no-store",
      "reload",
      "no-cache",
      "force-cache",
      "only-if-cached",
    ],
  );

  // https://fetch.spec.whatwg.org/#requestredirect
  const requestRedirectConverter = createEnumConverter(
    "RequestRedirect",
    [
      "follow",
      "error",
      "manual",
    ],
  );

  // https://dom.spec.whatwg.org/#abortsignal
  const abortSignalConverter = todo;

  // https://fetch.spec.whatwg.org/#requestinit
  const requestInitConverter = createDictionaryConverter("RequestInit", [
    {
      converter: converters.ByteString,
      key: "method",
    },
    {
      converter: headersInitConverter,
      key: "headers",
    },
    {
      converter: createNullableConverter(bodyInitConverter),
      key: "body",
    },
    /*{
      converter: converters.USVString,
      key: "referrer",
    }*/
    /*{
      converter: referrerPolicyConverter,
      key: "referrerPolicy",
    }*/
    /*{
      converter: requestModeConverter,
      key: "mode",
    }*/
    {
      converter: requestCredentialsConverter,
      key: "credentials",
    },
    /*{
      converter: requestCacheConverter,
      key: "cache",
    }*/
    {
      converter: requestRedirectConverter,
      key: "redirect",
    },
    /*{
      converter: converters.DOMString,
      key: "integrity",
    }*/
    /*{
      converter: converters.boolean,
      key: "keepalive",
    }*/
    /*{
      converter: createNullableConverter(abortSignalConverter),
      key: "signal",
	}*/
  ]);

  // https://fetch.spec.whatwg.org/#responseinit
  const responseInitConverter = createDictionaryConverter("ResponseInit", [
    {
      key: "status",
      defaultValue: 200,
      converter: converters["unsigned short"],
    },
    /*{
      key: "cancelable",
      defaultValue: "",
      converter: converters.ByteString,
    }*/
    {
      key: "headers",
      converter: headersInitConverter,
    },
  ]);

  class Request extends Body {
    /** @type {string} */
    #method = "GET";
    /** @type {string} */
    #url = "";
    /** @type {Headers} */
    #headers;
    /** @type {"include" | "omit" | "same-origin" | undefined} */
    #credentials = "omit";

    /**
     * @param {RequestInfo} input 
     * @param {RequestInit} init 
     */
    // @ts-expect-error because the use of super in this constructor is valid.
    constructor(input, init = {}) {
      requiredArguments("Request", arguments.length, 1);
      // input = requestInfoConverter(input);
      init = requestInitConverter(init, {
        prefix: "Failed to construct 'Request'",
      });

      let b;

      // prefer body from init
      if (init.body) {
        b = init.body;
      } else if (input instanceof Request) {
        if (input.bodyUsed) {
          throw TypeError(BodyUsedError);
        }
        b = input[teeBody]();
      } else if (typeof input === "object" && "body" in input && input.body) {
        if (input.bodyUsed) {
          throw TypeError(BodyUsedError);
        }
        b = input.body;
      } else {
        b = "";
      }

      let headers;
      // prefer headers from init
      if (init.headers) {
        headers = new Headers(init.headers);
      } else if (input instanceof Request) {
        headers = input.headers;
      } else {
        headers = new Headers();
      }

      const contentType = headers.get("content-type") || "";
      super(b, { contentType });
      this.#headers = headers;

      if (input instanceof Request) {
        if (input.bodyUsed) {
          throw TypeError(BodyUsedError);
        }
        this.#method = input.method;
        this.#url = input.url;
        this.#headers = new Headers(input.headers);
        this.#credentials = input.credentials;
      } else {
        // Constructing a URL just for validation is known to be expensive.
        // dontValidateUrl allows one to opt out.
        if (init[dontValidateUrl]) {
          this.#url = input;
        } else {
          const baseUrl = getLocationHref();
          this.#url = baseUrl != null
            ? new URL(String(input), baseUrl).href
            : new URL(String(input)).href;
        }
      }

      if (init && "method" in init && init.method) {
        this.#method = normalizeMethod(init.method);
      }

      if (
        init &&
        "credentials" in init &&
        init.credentials
      ) {
        this.credentials = init.credentials;
      }
    }

    clone() {
      if (this.bodyUsed) {
        throw new TypeError(BodyUsedError);
      }

      const iterators = this.headers.entries();
      const headersList = [...iterators];

      const body = this[teeBody]();

      return new Request(this.url, {
        body,
        method: this.method,
        headers: new Headers(headersList),
        credentials: this.credentials,
      });
    }

    get method() {
      return this.#method;
    }

    set method(_) {
      // can not set method
    }

    get url() {
      return this.#url;
    }

    set url(_) {
      // can not set url
    }

    get headers() {
      return this.#headers;
    }

    set headers(_) {
      // can not set headers
    }

    get credentials() {
      return this.#credentials;
    }

    set credentials(_) {
      // can not set credentials
    }
  }

  const responseData = new WeakMap();
  class Response extends Body {
    /** 
     * @param {BodyInit | null} body 
     * @param {ResponseInit} [init]
     */
    constructor(body = null, init = {}) {
      init = responseInitConverter(init, {
        prefix: "Failed to construct 'Response'",
      });

      const extraInit = responseData.get(init) || {};
      let { type = "default", url = "" } = extraInit;

      let status = init.status === undefined ? 200 : Number(init.status || 0);
      let statusText = init.statusText ?? "";
      let headers = init.headers instanceof Headers
        ? init.headers
        : new Headers(init.headers);

      if (init.status !== undefined && (status < 200 || status > 599)) {
        throw new RangeError(
          `The status provided (${init.status}) is outside the range [200, 599]`,
        );
      }

      // null body status
      if (body && NULL_BODY_STATUS.includes(status)) {
        throw new TypeError("Response with null body status cannot have body");
      }

      if (!type) {
        type = "default";
      } else {
        if (type == "error") {
          // spec: https://fetch.spec.whatwg.org/#concept-network-error
          status = 0;
          statusText = "";
          headers = new Headers();
          body = null;
          /* spec for other Response types:
           https://fetch.spec.whatwg.org/#concept-filtered-response-basic
           Please note that type "basic" is not the same thing as "default".*/
        } else if (type == "basic") {
          for (const h of headers) {
            /* Forbidden Response-Header Names:
             https://fetch.spec.whatwg.org/#forbidden-response-header-name */
            if (["set-cookie", "set-cookie2"].includes(h[0].toLowerCase())) {
              headers.delete(h[0]);
            }
          }
        } else if (type == "cors") {
          /* CORS-safelisted Response-Header Names:
             https://fetch.spec.whatwg.org/#cors-safelisted-response-header-name */
          const allowedHeaders = [
            "Cache-Control",
            "Content-Language",
            "Content-Length",
            "Content-Type",
            "Expires",
            "Last-Modified",
            "Pragma",
          ].map((c) => c.toLowerCase());
          for (const h of headers) {
            /* Technically this is still not standards compliant because we are
             supposed to allow headers allowed in the
             'Access-Control-Expose-Headers' header in the 'internal response'
             However, this implementation of response doesn't seem to have an
             easy way to access the internal response, so we ignore that
             header.
             TODO(serverhiccups): change how internal responses are handled
             so we can do this properly. */
            if (!allowedHeaders.includes(h[0].toLowerCase())) {
              headers.delete(h[0]);
            }
          }
          /* TODO(serverhiccups): Once I fix the 'internal response' thing,
           these actually need to treat the internal response differently */
        } else if (type == "opaque" || type == "opaqueredirect") {
          url = "";
          status = 0;
          statusText = "";
          headers = new Headers();
          body = null;
        }
      }

      const contentType = headers.get("content-type") || "";
      const size = Number(headers.get("content-length")) || undefined;

      super(body, { contentType, size });

      this.url = url;
      this.statusText = statusText;
      this.status = extraInit.status || status;
      this.headers = headers;
      this.redirected = extraInit.redirected || false;
      this.type = type;
    }

    get ok() {
      return 200 <= this.status && this.status < 300;
    }

    clone() {
      if (this.bodyUsed) {
        throw TypeError(BodyUsedError);
      }

      const iterators = this.headers.entries();
      const headersList = [];
      for (const header of iterators) {
        headersList.push(header);
      }

      const body = this[teeBody]();

      return new Response(body, {
        status: this.status,
        statusText: this.statusText,
        headers: new Headers(headersList),
      });
    }

    /**
     * @param {string } url 
     * @param {number} status
     */
    static redirect(url, status = 302) {
      if (![301, 302, 303, 307, 308].includes(status)) {
        throw new RangeError(
          "The redirection status must be one of 301, 302, 303, 307 and 308.",
        );
      }
      return new Response(null, {
        status,
        statusText: "",
        headers: [["Location", String(url)]],
      });
    }
  }

  /** @type {string | null} */
  let baseUrl = null;

  /** @param {string} href */
  function setBaseUrl(href) {
    baseUrl = href;
  }

  /**
   * @param {string} url 
   * @param {string} method 
   * @param {Headers} headers 
   * @param {ReadableStream<Uint8Array> | ArrayBufferView | undefined} body 
   * @param {number | null} clientRid
   * @returns {Promise<{status: number, statusText: string, headers: Record<string,string[]>, url: string, responseRid: number}>}
   */
  async function sendFetchReq(url, method, headers, body, clientRid) {
    /** @type {[string, string][]} */
    let headerArray = [];
    if (headers) {
      headerArray = Array.from(headers.entries());
    }

    const { requestRid, requestBodyRid } = opFetch(
      {
        method,
        url,
        baseUrl,
        headers: headerArray,
        clientRid,
        hasBody: !!body,
      },
      body instanceof Uint8Array ? body : null,
    );
    if (requestBodyRid) {
      if (!(body instanceof ReadableStream)) {
        throw new TypeError("Unreachable state (body is not ReadableStream).");
      }
      const writer = new WritableStream({
        /**
         * @param {Uint8Array} chunk 
         * @param {WritableStreamDefaultController} controller 
         */
        async write(chunk, controller) {
          try {
            await opFetchRequestWrite({ rid: requestBodyRid }, chunk);
          } catch (err) {
            controller.error(err);
          }
        },
        close() {
          core.close(requestBodyRid);
        },
      });
      body.pipeTo(writer);
    }

    return await opFetchSend({ rid: requestRid });
  }

  /**
   * @param {Request | URL | string} input 
   * @param {RequestInit & {client: Deno.HttpClient}} [init] 
   * @returns {Promise<Response>}
   */
  async function fetch(input, init = {}) {
    requiredArguments("fetch", arguments.length, 1);
    // input = requestInfoConverter(input);
    init = requestInitConverter(init, { prefix: "Failed to execute 'fetch'" });

    let url;
    let method = null;
    let headers = null;
    let body;

    let clientRid = null;
    let redirected = false;
    let remRedirectCount = 20; // TODO(bartlomieju): use a better way to handle

    if (typeof input === "string" || input instanceof URL) {
      url = typeof input === "string" ? input : input.href;
      if (init != null) {
        method = init.method || null;
        if (init.headers) {
          headers = init.headers instanceof Headers
            ? init.headers
            : new Headers(init.headers);
        } else {
          headers = null;
        }

        // ref: https://fetch.spec.whatwg.org/#body-mixin
        // Body should have been a mixin
        // but we are treating it as a separate class
        if (init.body) {
          if (!headers) {
            headers = new Headers();
          }
          let contentType = "";
          if (typeof init.body === "string") {
            body = new TextEncoder().encode(init.body);
            contentType = "text/plain;charset=UTF-8";
          } else if (isTypedArray(init.body)) {
            body = init.body;
          } else if (init.body instanceof ArrayBuffer) {
            body = new Uint8Array(init.body);
          } else if (init.body instanceof URLSearchParams) {
            body = new TextEncoder().encode(init.body.toString());
            contentType = "application/x-www-form-urlencoded;charset=UTF-8";
          }
          if (init.body instanceof Blob) {
            ({
              [_byteSequence]: body,
              type: contentType,
            } = init.body);
          } else if (init.body instanceof FormData) {
            let boundary;
            if (headers.has("content-type")) {
              const params = getHeaderValueParams("content-type");
              boundary = params.get("boundary");
            }
            const multipartBuilder = new MultipartBuilder(
              init.body,
              boundary,
            );
            body = multipartBuilder.getBody();
            contentType = multipartBuilder.getContentType();
          } else if (init.body instanceof ReadableStream) {
            body = init.body;
          }
          if (contentType && !headers.has("content-type")) {
            headers.set("content-type", contentType);
          }
        }
        if (init.client instanceof HttpClient) {
          clientRid = init.client.rid;
        }
      }
    } else {
      url = input.url;
      method = input.method;
      headers = input.headers;

      if (input.body) {
        body = input.body;
      }
    }

    let responseBody;
    let responseInit = {};
    while (remRedirectCount) {
      const fetchResp = await sendFetchReq(
        url,
        method ?? "GET",
        headers ?? new Headers(),
        body,
        clientRid,
      );
      const rid = fetchResp.responseRid;

      if (
        NULL_BODY_STATUS.includes(fetchResp.status) ||
        REDIRECT_STATUS.includes(fetchResp.status)
      ) {
        // We won't use body of received response, so close it now
        // otherwise it will be kept in resource table.
        core.close(rid);
        responseBody = null;
      } else {
        responseBody = new ReadableStream({
          type: "bytes",
          /** @param {ReadableStreamDefaultController<Uint8Array>} controller */
          async pull(controller) {
            try {
              const chunk = new Uint8Array(16 * 1024 + 256);
              const { read } = await core.jsonOpAsync(
                "op_fetch_response_read",
                { rid },
                chunk,
              );
              if (read != 0) {
                if (chunk.length == read) {
                  controller.enqueue(chunk);
                } else {
                  controller.enqueue(chunk.subarray(0, read));
                }
              } else {
                controller.close();
                core.close(rid);
              }
            } catch (e) {
              controller.error(e);
              controller.close();
              core.close(rid);
            }
          },
          cancel() {
            // When reader.cancel() is called
            core.close(rid);
          },
        });
      }

      responseInit = {
        status: 200,
        statusText: fetchResp.statusText,
        headers: fetchResp.headers,
      };

      responseData.set(responseInit, {
        redirected,
        rid: fetchResp.responseRid,
        status: fetchResp.status,
        url: fetchResp.url,
      });

      const response = new Response(responseBody, responseInit);

      if (REDIRECT_STATUS.includes(fetchResp.status)) {
        // We're in a redirect status
        switch ((init && init.redirect) || "follow") {
          case "error":
            responseInit = {};
            responseData.set(responseInit, {
              type: "error",
              redirected: false,
              url: "",
            });
            return new Response(null, responseInit);
          case "manual":
            // On the web this would return a `opaqueredirect` response, but
            // those don't make sense server side. See denoland/deno#8351.
            return response;
          case "follow":
          // fallthrough
          default: {
            /** @type {string | null} */
            let redirectUrl = response.headers.get("Location");
            if (redirectUrl == null) {
              return response; // Unspecified
            }
            if (
              !redirectUrl.startsWith("http://") &&
              !redirectUrl.startsWith("https://")
            ) {
              redirectUrl = new URL(redirectUrl, fetchResp.url).href;
            }
            url = redirectUrl;
            redirected = true;
            remRedirectCount--;
          }
        }
      } else {
        return response;
      }
    }

    responseData.set(responseInit, {
      type: "error",
      redirected: false,
      url: "",
    });

    return new Response(null, responseInit);
  }

  window.__bootstrap.fetch = {
    FormData,
    setBaseUrl,
    fetch,
    Request,
    Response,
    HttpClient,
    createHttpClient,
    fastBody,
    dontValidateUrl,
  };
})(this);
