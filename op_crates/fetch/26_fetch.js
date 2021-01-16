// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  // provided by "deno_web"
  const { URLSearchParams } = window.__bootstrap.url;
  const { getLocationHref } = window.__bootstrap.location;

  const { requiredArguments } = window.__bootstrap.fetchUtil;
  const { ReadableStream, isReadableStreamDisturbed } =
    window.__bootstrap.streams;
  const { DomIterableMixin } = window.__bootstrap.domIterable;
  const { Headers } = window.__bootstrap.headers;

  // FIXME(bartlomieju): stubbed out, needed in blob
  const build = {
    os: "",
  };

  const MAX_SIZE = 2 ** 32 - 2;

  // `off` is the offset into `dst` where it will at which to begin writing values
  // from `src`.
  // Returns the number of bytes copied.
  function copyBytes(src, dst, off = 0) {
    const r = dst.byteLength - off;
    if (src.byteLength > r) {
      src = src.subarray(0, r);
    }
    dst.set(src, off);
    return src.byteLength;
  }

  class Buffer {
    #buf = null; // contents are the bytes buf[off : len(buf)]
    #off = 0; // read at buf[off], write at buf[buf.byteLength]

    constructor(ab) {
      if (ab == null) {
        this.#buf = new Uint8Array(0);
        return;
      }

      this.#buf = new Uint8Array(ab);
    }

    bytes(options = { copy: true }) {
      if (options.copy === false) return this.#buf.subarray(this.#off);
      return this.#buf.slice(this.#off);
    }

    empty() {
      return this.#buf.byteLength <= this.#off;
    }

    get length() {
      return this.#buf.byteLength - this.#off;
    }

    get capacity() {
      return this.#buf.buffer.byteLength;
    }

    reset() {
      this.#reslice(0);
      this.#off = 0;
    }

    #tryGrowByReslice = (n) => {
      const l = this.#buf.byteLength;
      if (n <= this.capacity - l) {
        this.#reslice(l + n);
        return l;
      }
      return -1;
    };

    #reslice = (len) => {
      if (!(len <= this.#buf.buffer.byteLength)) {
        throw new Error("assert");
      }
      this.#buf = new Uint8Array(this.#buf.buffer, 0, len);
    };

    writeSync(p) {
      const m = this.#grow(p.byteLength);
      return copyBytes(p, this.#buf, m);
    }

    write(p) {
      const n = this.writeSync(p);
      return Promise.resolve(n);
    }

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

    grow(n) {
      if (n < 0) {
        throw Error("Buffer.grow: negative count");
      }
      const m = this.#grow(n);
      this.#reslice(m);
    }
  }

  function isTypedArray(x) {
    return ArrayBuffer.isView(x) && !(x instanceof DataView);
  }

  function hasHeaderValueOf(s, value) {
    return new RegExp(`^${value}(?:[\\s;]|$)`).test(s);
  }

  function getHeaderValueParams(value) {
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
  const bytesSymbol = Symbol("bytes");

  function containsOnlyASCII(str) {
    if (typeof str !== "string") {
      return false;
    }
    // deno-lint-ignore no-control-regex
    return /^[\x00-\x7F]*$/.test(str);
  }

  function convertLineEndingsToNative(s) {
    const nativeLineEnd = build.os == "windows" ? "\r\n" : "\n";

    let position = 0;

    let collectionResult = collectSequenceNotCRLF(s, position);

    let token = collectionResult.collected;
    position = collectionResult.newPosition;

    let result = token;

    while (position < s.length) {
      const c = s.charAt(position);
      if (c == "\r") {
        result += nativeLineEnd;
        position++;
        if (position < s.length && s.charAt(position) == "\n") {
          position++;
        }
      } else if (c == "\n") {
        position++;
        result += nativeLineEnd;
      }

      collectionResult = collectSequenceNotCRLF(s, position);

      token = collectionResult.collected;
      position = collectionResult.newPosition;

      result += token;
    }

    return result;
  }

  function collectSequenceNotCRLF(
    s,
    position,
  ) {
    const start = position;
    for (
      let c = s.charAt(position);
      position < s.length && !(c == "\r" || c == "\n");
      c = s.charAt(++position)
    );
    return { collected: s.slice(start, position), newPosition: position };
  }

  function toUint8Arrays(
    blobParts,
    doNormalizeLineEndingsToNative,
  ) {
    const ret = [];
    const enc = new TextEncoder();
    for (const element of blobParts) {
      if (typeof element === "string") {
        let str = element;
        if (doNormalizeLineEndingsToNative) {
          str = convertLineEndingsToNative(element);
        }
        ret.push(enc.encode(str));
        // eslint-disable-next-line @typescript-eslint/no-use-before-define
      } else if (element instanceof Blob) {
        ret.push(element[bytesSymbol]);
      } else if (element instanceof Uint8Array) {
        ret.push(element);
      } else if (element instanceof Uint16Array) {
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (element instanceof Uint32Array) {
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (ArrayBuffer.isView(element)) {
        // Convert view to Uint8Array.
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (element instanceof ArrayBuffer) {
        // Create a new Uint8Array view for the given ArrayBuffer.
        const uint8 = new Uint8Array(element);
        ret.push(uint8);
      } else {
        ret.push(enc.encode(String(element)));
      }
    }
    return ret;
  }

  function processBlobParts(
    blobParts,
    options,
  ) {
    const normalizeLineEndingsToNative = options.ending === "native";
    // ArrayBuffer.transfer is not yet implemented in V8, so we just have to
    // pre compute size of the array buffer and do some sort of static allocation
    // instead of dynamic allocation.
    const uint8Arrays = toUint8Arrays(blobParts, normalizeLineEndingsToNative);
    const byteLength = uint8Arrays
      .map((u8) => u8.byteLength)
      .reduce((a, b) => a + b, 0);
    const ab = new ArrayBuffer(byteLength);
    const bytes = new Uint8Array(ab);
    let courser = 0;
    for (const u8 of uint8Arrays) {
      bytes.set(u8, courser);
      courser += u8.byteLength;
    }

    return bytes;
  }

  function getStream(blobBytes) {
    // TODO: Align to spec https://fetch.spec.whatwg.org/#concept-construct-readablestream
    return new ReadableStream({
      type: "bytes",
      start: (controller) => {
        controller.enqueue(blobBytes);
        controller.close();
      },
    });
  }

  async function readBytes(
    reader,
  ) {
    const chunks = [];
    while (true) {
      const { done, value } = await reader.read();
      if (!done && value instanceof Uint8Array) {
        chunks.push(value);
      } else if (done) {
        const size = chunks.reduce((p, i) => p + i.byteLength, 0);
        const bytes = new Uint8Array(size);
        let offs = 0;
        for (const chunk of chunks) {
          bytes.set(chunk, offs);
          offs += chunk.byteLength;
        }
        return bytes.buffer;
      } else {
        throw new TypeError("Invalid reader result.");
      }
    }
  }

  // A WeakMap holding blob to byte array mapping.
  // Ensures it does not impact garbage collection.
  // const blobBytesWeakMap = new WeakMap();

  class Blob {
    constructor(blobParts, options) {
      if (arguments.length === 0) {
        this[bytesSymbol] = new Uint8Array();
        return;
      }

      const { ending = "transparent", type = "" } = options ?? {};
      // Normalize options.type.
      let normalizedType = type;
      if (!containsOnlyASCII(type)) {
        normalizedType = "";
      } else {
        if (type.length) {
          for (let i = 0; i < type.length; ++i) {
            const char = type[i];
            if (char < "\u0020" || char > "\u007E") {
              normalizedType = "";
              break;
            }
          }
          normalizedType = type.toLowerCase();
        }
      }
      const bytes = processBlobParts(blobParts, { ending, type });
      // Set Blob object's properties.
      this[bytesSymbol] = bytes;
      this.size = bytes.byteLength;
      this.type = normalizedType;
    }

    slice(start, end, contentType) {
      return new Blob([this[bytesSymbol].slice(start, end)], {
        type: contentType || this.type,
      });
    }

    stream() {
      return getStream(this[bytesSymbol]);
    }

    async text() {
      const reader = getStream(this[bytesSymbol]).getReader();
      const decoder = new TextDecoder();
      return decoder.decode(await readBytes(reader));
    }

    arrayBuffer() {
      return readBytes(getStream(this[bytesSymbol]).getReader());
    }
  }

  class DomFile extends Blob {
    constructor(
      fileBits,
      fileName,
      options,
    ) {
      const { lastModified = Date.now(), ...blobPropertyBag } = options ?? {};
      super(fileBits, blobPropertyBag);

      // 4.1.2.1 Replace any "/" character (U+002F SOLIDUS)
      // with a ":" (U + 003A COLON)
      this.name = String(fileName).replace(/\u002F/g, "\u003A");
      // 4.1.3.3 If lastModified is not provided, set lastModified to the current
      // date and time represented in number of milliseconds since the Unix Epoch.
      this.lastModified = lastModified;
    }
  }

  function parseFormDataValue(value, filename) {
    if (value instanceof DomFile) {
      return new DomFile([value], filename || value.name, {
        type: value.type,
        lastModified: value.lastModified,
      });
    } else if (value instanceof Blob) {
      return new DomFile([value], filename || "blob", {
        type: value.type,
      });
    } else {
      return String(value);
    }
  }

  class FormDataBase {
    [dataSymbol] = [];

    append(name, value, filename) {
      requiredArguments("FormData.append", arguments.length, 2);
      name = String(name);
      this[dataSymbol].push([name, parseFormDataValue(value, filename)]);
    }

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

    has(name) {
      requiredArguments("FormData.has", arguments.length, 1);
      name = String(name);
      return this[dataSymbol].some((entry) => entry[0] === name);
    }

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
    constructor(formData, boundary) {
      this.formData = formData;
      this.boundary = boundary ?? this.#createBoundary();
      this.writer = new Buffer();
    }

    getContentType() {
      return `multipart/form-data; boundary=${this.boundary}`;
    }

    getBody() {
      for (const [fieldName, fieldValue] of this.formData.entries()) {
        if (fieldValue instanceof DomFile) {
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

    #writeHeaders = (headers) => {
      let buf = this.writer.empty() ? "" : "\r\n";

      buf += `--${this.boundary}\r\n`;
      for (const [key, value] of headers) {
        buf += `${key}: ${value}\r\n`;
      }
      buf += `\r\n`;

      // FIXME(Bartlomieju): this should use `writeSync()`
      this.writer.write(encoder.encode(buf));
    };

    #writeFileHeaders = (
      field,
      filename,
      type,
    ) => {
      const headers = [
        [
          "Content-Disposition",
          `form-data; name="${field}"; filename="${filename}"`,
        ],
        ["Content-Type", type || "application/octet-stream"],
      ];
      return this.#writeHeaders(headers);
    };

    #writeFieldHeaders = (field) => {
      const headers = [["Content-Disposition", `form-data; name="${field}"`]];
      return this.#writeHeaders(headers);
    };

    #writeField = (field, value) => {
      this.#writeFieldHeaders(field);
      this.writer.writeSync(encoder.encode(value));
    };

    #writeFile = (field, value) => {
      this.#writeFileHeaders(field, value.name, value.type);
      this.writer.writeSync(value[bytesSymbol]);
    };
  }

  class MultipartParser {
    constructor(body, boundary) {
      if (!boundary) {
        throw new TypeError("multipart/form-data must provide a boundary");
      }

      this.boundary = `--${boundary}`;
      this.body = body;
      this.boundaryChars = encoder.encode(this.boundary);
    }

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

  function validateBodyType(owner, bodySource) {
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
    throw new Error(
      `Bad ${owner.constructor.name} body type: ${bodySource.constructor.name}`,
    );
  }

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

  function bodyToArrayBuffer(bodySource) {
    if (isTypedArray(bodySource)) {
      return bodySource.buffer;
    } else if (bodySource instanceof ArrayBuffer) {
      return bodySource;
    } else if (typeof bodySource === "string") {
      const enc = new TextEncoder();
      return enc.encode(bodySource).buffer;
    } else if (bodySource instanceof ReadableStream) {
      throw new Error(
        `Can't convert stream to ArrayBuffer (try bufferFromStream)`,
      );
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

  class Body {
    #contentType = "";
    #size = undefined;

    constructor(_bodySource, meta) {
      validateBodyType(this, _bodySource);
      this._bodySource = _bodySource;
      this.#contentType = meta.contentType;
      this.#size = meta.size;
      this._stream = null;
    }

    get body() {
      if (this._stream) {
        return this._stream;
      }

      if (!this._bodySource) {
        return null;
      } else if (this._bodySource instanceof ReadableStream) {
        this._stream = this._bodySource;
      } else {
        const buf = bodyToArrayBuffer(this._bodySource);
        if (!(buf instanceof ArrayBuffer)) {
          throw new Error(
            `Expected ArrayBuffer from body`,
          );
        }

        this._stream = new ReadableStream({
          start(controller) {
            controller.enqueue(new Uint8Array(buf));
            controller.close();
          },
        });
      }

      return this._stream;
    }

    get bodyUsed() {
      if (this.body && isReadableStreamDisturbed(this.body)) {
        return true;
      }
      return false;
    }

    async blob() {
      return new Blob([await this.arrayBuffer()], {
        type: this.#contentType,
      });
    }

    // ref: https://fetch.spec.whatwg.org/#body-mixin
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
                const name = split.shift().replace(/\+/g, " ");
                const value = split.join("=").replace(/\+/g, " ");
                formData.append(
                  decodeURIComponent(name),
                  decodeURIComponent(value),
                );
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

    async text() {
      if (typeof this._bodySource === "string") {
        return this._bodySource;
      }

      const ab = await this.arrayBuffer();
      const decoder = new TextDecoder("utf-8");
      return decoder.decode(ab);
    }

    async json() {
      const raw = await this.text();
      return JSON.parse(raw);
    }

    arrayBuffer() {
      if (this._bodySource instanceof ReadableStream) {
        return bufferFromStream(this._bodySource.getReader(), this.#size);
      }
      return Promise.resolve(bodyToArrayBuffer(this._bodySource));
    }
  }

  function createHttpClient(options) {
    return new HttpClient(opCreateHttpClient(options));
  }

  function opCreateHttpClient(args) {
    return core.jsonOpSync("op_create_http_client", args);
  }

  class HttpClient {
    constructor(rid) {
      this.rid = rid;
    }
    close() {
      core.close(this.rid);
    }
  }

  function opFetch(args, body) {
    let zeroCopy;
    if (body != null) {
      zeroCopy = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
    }
    return core.jsonOpSync("op_fetch", args, ...(zeroCopy ? [zeroCopy] : []));
  }

  function opFetchSend(args) {
    return core.jsonOpAsync("op_fetch_send", args);
  }

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

  function byteUpperCase(s) {
    return String(s).replace(/[a-z]/g, function byteUpperCaseReplace(c) {
      return c.toUpperCase();
    });
  }

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

  class Request extends Body {
    constructor(input, init) {
      if (arguments.length < 1) {
        throw TypeError("Not enough arguments");
      }

      if (!init) {
        init = {};
      }

      let b;

      // prefer body from init
      if (init.body) {
        b = init.body;
      } else if (input instanceof Request && input._bodySource) {
        if (input.bodyUsed) {
          throw TypeError(BodyUsedError);
        }
        b = input._bodySource;
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
      this.headers = headers;

      // readonly attribute ByteString method;
      this.method = "GET";

      // readonly attribute USVString url;
      this.url = "";

      // readonly attribute RequestCredentials credentials;
      this.credentials = "omit";

      if (input instanceof Request) {
        if (input.bodyUsed) {
          throw TypeError(BodyUsedError);
        }
        this.method = input.method;
        this.url = input.url;
        this.headers = new Headers(input.headers);
        this.credentials = input.credentials;
        this._stream = input._stream;
      } else {
        const baseUrl = getLocationHref();
        this.url = baseUrl != null
          ? new URL(String(input), baseUrl).href
          : new URL(String(input)).href;
      }

      if (init && "method" in init && init.method) {
        this.method = normalizeMethod(init.method);
      }

      if (
        init &&
        "credentials" in init &&
        init.credentials &&
        ["omit", "same-origin", "include"].indexOf(init.credentials) !== -1
      ) {
        this.credentials = init.credentials;
      }
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

      let body2 = this._bodySource;

      if (this._bodySource instanceof ReadableStream) {
        const tees = this._bodySource.tee();
        this._stream = this._bodySource = tees[0];
        body2 = tees[1];
      }

      return new Request(this.url, {
        body: body2,
        method: this.method,
        headers: new Headers(headersList),
        credentials: this.credentials,
      });
    }
  }

  const responseData = new WeakMap();
  class Response extends Body {
    constructor(body = null, init) {
      init = init ?? {};

      if (typeof init !== "object") {
        throw new TypeError(`'init' is not an object`);
      }

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

      let resBody = this._bodySource;

      if (this._bodySource instanceof ReadableStream) {
        const tees = this._bodySource.tee();
        this._stream = this._bodySource = tees[0];
        resBody = tees[1];
      }

      return new Response(resBody, {
        status: this.status,
        statusText: this.statusText,
        headers: new Headers(headersList),
      });
    }

    static redirect(url, status) {
      if (![301, 302, 303, 307, 308].includes(status)) {
        throw new RangeError(
          "The redirection status must be one of 301, 302, 303, 307 and 308.",
        );
      }
      return new Response(null, {
        status,
        statusText: "",
        headers: [["Location", typeof url === "string" ? url : url.toString()]],
      });
    }
  }

  let baseUrl = null;

  function setBaseUrl(href) {
    baseUrl = href;
  }

  async function sendFetchReq(url, method, headers, body, clientRid) {
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
      body instanceof Uint8Array ? body : undefined,
    );
    if (requestBodyRid) {
      const writer = new WritableStream({
        async write(chunk, controller) {
          try {
            await opFetchRequestWrite({ rid: requestBodyRid }, chunk);
          } catch (err) {
            controller.error(err);
            controller.close();
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

  async function fetch(input, init) {
    let url;
    let method = null;
    let headers = null;
    let body;
    let clientRid = null;
    let redirected = false;
    let remRedirectCount = 20; // TODO: use a better way to handle

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
          } else if (init.body instanceof Blob) {
            body = init.body[bytesSymbol];
            contentType = init.body.type;
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
      const fetchResponse = await sendFetchReq(
        url,
        method,
        headers,
        body,
        clientRid,
      );
      const rid = fetchResponse.responseRid;

      if (
        NULL_BODY_STATUS.includes(fetchResponse.status) ||
        REDIRECT_STATUS.includes(fetchResponse.status)
      ) {
        // We won't use body of received response, so close it now
        // otherwise it will be kept in resource table.
        core.close(rid);
        responseBody = null;
      } else {
        responseBody = new ReadableStream({
          type: "bytes",
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
        statusText: fetchResponse.statusText,
        headers: fetchResponse.headers,
      };

      responseData.set(responseInit, {
        redirected,
        rid: fetchResponse.bodyRid,
        status: fetchResponse.status,
        url,
      });

      const response = new Response(responseBody, responseInit);

      if (REDIRECT_STATUS.includes(fetchResponse.status)) {
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
            let redirectUrl = response.headers.get("Location");
            if (redirectUrl == null) {
              return response; // Unspecified
            }
            if (
              !redirectUrl.startsWith("http://") &&
              !redirectUrl.startsWith("https://")
            ) {
              redirectUrl = new URL(redirectUrl, url).href;
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
    Blob,
    DomFile,
    FormData,
    setBaseUrl,
    fetch,
    Request,
    Response,
    HttpClient,
    createHttpClient,
  };
})(this);
