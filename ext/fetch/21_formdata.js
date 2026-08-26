// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  Blob,
  BlobPrototype,
  File,
  FilePrototype,
} = core.loadExtScript("ext:deno_web/09_file.js");
const {
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  MathRandom,
  ObjectFreeze,
  ObjectFromEntries,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SafeMap,
  SafeRegExp,
  Symbol,
  SymbolFor,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  StringPrototypeTrim,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeReplace,
  StringPrototypeIndexOf,
  StringPrototypePadStart,
  StringPrototypeCodePointAt,
  StringPrototypeReplaceAll,
  TypeError,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

const entryList = Symbol("entry list");

/**
 * @param {string} name
 * @param {string | Blob} value
 * @param {string | undefined} filename
 * @returns {FormDataEntry}
 */
function createEntry(name, value, filename) {
  if (
    ObjectPrototypeIsPrototypeOf(BlobPrototype, value) &&
    !ObjectPrototypeIsPrototypeOf(FilePrototype, value)
  ) {
    value = new File([value], "blob", { type: value.type });
  }
  if (
    ObjectPrototypeIsPrototypeOf(FilePrototype, value) &&
    filename !== undefined
  ) {
    value = new File([value], filename, {
      type: value.type,
      lastModified: value.lastModified,
    });
  }
  return {
    name,
    // @ts-expect-error because TS is not smart enough
    value,
  };
}

/**
 * @typedef FormDataEntry
 * @property {string} name
 * @property {FormDataEntryValue} value
 */

class FormData {
  /** @type {FormDataEntry[]} */
  [entryList] = [];

  /** @param {void} form */
  constructor(form) {
    if (form !== undefined) {
      webidl.illegalConstructor();
    }
    this[webidl.brand] = webidl.brand;
  }

  /**
   * @param {string} name
   * @param {string | Blob} valueOrBlobValue
   * @param {string} [filename]
   * @returns {void}
   */
  append(name, valueOrBlobValue, filename) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'append' on 'FormData'";
    webidl.requiredArguments(arguments.length, 2, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");
    if (ObjectPrototypeIsPrototypeOf(BlobPrototype, valueOrBlobValue)) {
      valueOrBlobValue = webidl.converters["Blob"](
        valueOrBlobValue,
        prefix,
        "Argument 2",
      );
      if (filename !== undefined) {
        filename = webidl.converters["USVString"](
          filename,
          prefix,
          "Argument 3",
        );
      }
    } else {
      valueOrBlobValue = webidl.converters["USVString"](
        valueOrBlobValue,
        prefix,
        "Argument 2",
      );
    }

    const entry = createEntry(name, valueOrBlobValue, filename);

    ArrayPrototypePush(this[entryList], entry);
  }

  /**
   * @param {string} name
   * @returns {void}
   */
  delete(name) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'name' on 'FormData'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");

    const list = this[entryList];
    let writeIdx = 0;
    for (let i = 0; i < list.length; i++) {
      if (list[i].name !== name) {
        list[writeIdx++] = list[i];
      }
    }
    if (writeIdx !== list.length) {
      ArrayPrototypeSplice(list, writeIdx);
    }
  }

  /**
   * @param {string} name
   * @returns {FormDataEntryValue | null}
   */
  get(name) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'get' on 'FormData'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");

    const entries = this[entryList];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      if (entry.name === name) return entry.value;
    }
    return null;
  }

  /**
   * @param {string} name
   * @returns {FormDataEntryValue[]}
   */
  getAll(name) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'getAll' on 'FormData'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");

    const returnList = [];
    const entries = this[entryList];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      if (entry.name === name) ArrayPrototypePush(returnList, entry.value);
    }
    return returnList;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  has(name) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'has' on 'FormData'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");

    const entries = this[entryList];
    for (let i = 0; i < entries.length; ++i) {
      const entry = entries[i];
      if (entry.name === name) return true;
    }
    return false;
  }

  /**
   * @param {string} name
   * @param {string | Blob} valueOrBlobValue
   * @param {string} [filename]
   * @returns {void}
   */
  set(name, valueOrBlobValue, filename) {
    webidl.assertBranded(this, FormDataPrototype);
    const prefix = "Failed to execute 'set' on 'FormData'";
    webidl.requiredArguments(arguments.length, 2, prefix);

    name = webidl.converters["USVString"](name, prefix, "Argument 1");
    if (ObjectPrototypeIsPrototypeOf(BlobPrototype, valueOrBlobValue)) {
      valueOrBlobValue = webidl.converters["Blob"](
        valueOrBlobValue,
        prefix,
        "Argument 2",
      );
      if (filename !== undefined) {
        filename = webidl.converters["USVString"](
          filename,
          prefix,
          "Argument 3",
        );
      }
    } else {
      valueOrBlobValue = webidl.converters["USVString"](
        valueOrBlobValue,
        prefix,
        "Argument 2",
      );
    }

    const entry = createEntry(name, valueOrBlobValue, filename);

    const list = this[entryList];
    let writeIdx = 0;
    let added = false;
    for (let i = 0; i < list.length; i++) {
      if (list[i].name === name) {
        if (!added) {
          list[writeIdx++] = entry;
          added = true;
        }
      } else {
        list[writeIdx++] = list[i];
      }
    }
    if (!added) {
      ArrayPrototypePush(list, entry);
    } else if (writeIdx !== list.length) {
      ArrayPrototypeSplice(list, writeIdx);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    if (ObjectPrototypeIsPrototypeOf(FormDataPrototype, this)) {
      return `${this.constructor.name} ${
        inspect(ObjectFromEntries(this), inspectOptions)
      }`;
    } else {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    }
  }
}

webidl.mixinPairIterable("FormData", FormData, entryList, "name", "value");

webidl.configureInterface(FormData);
const FormDataPrototype = FormData.prototype;

const ESCAPE_FILENAME_PATTERN = new SafeRegExp(/\r?\n|\r/g);
const ESCAPE_PATTERN = new SafeRegExp(/([\n\r"])/g);
const ESCAPE_MAP = ObjectFreeze({
  "\n": "%0A",
  "\r": "%0D",
  '"': "%22",
});

function escape(str, isFilename) {
  return StringPrototypeReplace(
    isFilename
      ? str
      : StringPrototypeReplace(str, ESCAPE_FILENAME_PATTERN, "\r\n"),
    ESCAPE_PATTERN,
    (c) => ESCAPE_MAP[c],
  );
}

const FORM_DETA_SERIALIZE_PATTERN = new SafeRegExp(/\r(?!\n)|(?<!\r)\n/g);

/**
 * convert FormData to a Blob synchronous without reading all of the files
 * @param {globalThis.FormData} formData
 */
function formDataToBlob(formData) {
  const boundary = StringPrototypePadStart(
    StringPrototypeSlice(
      StringPrototypeReplaceAll(`${MathRandom()}${MathRandom()}`, ".", ""),
      -28,
    ),
    32,
    "-",
  );
  const chunks = [];
  const prefix = `--${boundary}\r\nContent-Disposition: form-data; name="`;

  // deno-lint-ignore deno-internal/prefer-primordials
  for (const { 0: name, 1: value } of formData) {
    if (typeof value === "string") {
      ArrayPrototypePush(
        chunks,
        prefix + escape(name) + '"' + CRLF + CRLF +
          StringPrototypeReplace(
            value,
            FORM_DETA_SERIALIZE_PATTERN,
            CRLF,
          ) + CRLF,
      );
    } else {
      ArrayPrototypePush(
        chunks,
        prefix + escape(name) + `"; filename="${escape(value.name, true)}"` +
          CRLF +
          `Content-Type: ${value.type || "application/octet-stream"}\r\n\r\n`,
        value,
        CRLF,
      );
    }
  }

  ArrayPrototypePush(chunks, `--${boundary}--`);

  return new Blob(chunks, {
    type: "multipart/form-data; boundary=" + boundary,
  });
}

const QUOTE_CONTENT_PATTERN = new SafeRegExp(/^"([^"]*)"$/);

/**
 * @param {string} value
 * @returns {Map<string, string>}
 */
function parseContentDisposition(value) {
  /** @type {Map<string, string>} */
  const params = new SafeMap();
  // Forced to do so for some Map constructor param mismatch
  const values = ArrayPrototypeSlice(StringPrototypeSplit(value, ";"), 1);
  for (let i = 0; i < values.length; i++) {
    const entries = StringPrototypeSplit(StringPrototypeTrim(values[i]), "=");
    if (entries.length > 1) {
      MapPrototypeSet(
        params,
        entries[0],
        StringPrototypeReplace(entries[1], QUOTE_CONTENT_PATTERN, "$1"),
      );
    }
  }
  return params;
}

/**
 * Decodes a string containing UTF-8 mistakenly decoded as Latin-1 and
 * decodes it correctly.
 * @param {string} latin1String
 * @returns {string}
 */
function decodeLatin1StringAsUtf8(latin1String) {
  const buffer = new Uint8Array(latin1String.length);
  for (let i = 0; i < latin1String.length; i++) {
    buffer[i] = StringPrototypeCharCodeAt(latin1String, i);
  }
  return core.decode(buffer);
}

const CRLF = "\r\n";
const LF = StringPrototypeCodePointAt(CRLF, 1);
const CR = StringPrototypeCodePointAt(CRLF, 0);
const DASH = StringPrototypeCodePointAt("-", 0);
const SPACE = StringPrototypeCodePointAt(" ", 0);
const TAB = StringPrototypeCodePointAt("\t", 0);
const MAX_MULTIPART_PART_HEADER_SIZE = 16 * 1024;
const MAX_MULTIPART_PART_HEADER_COUNT = 128;

/**
 * @param {Uint8Array} bytes
 * @returns {string}
 */
function decodeLatin1Bytes(bytes) {
  return ReflectApply(StringFromCharCode, null, bytes);
}

class MultipartParser {
  /**
   * @param {Uint8Array} body
   * @param {string | undefined} boundary
   */
  constructor(body, boundary) {
    if (!boundary) {
      throw new TypeError(
        "Cannot construct MultipartParser: multipart/form-data must provide a boundary",
      );
    }

    this.body = body;
    this.boundaryChars = core.encode(`--${boundary}`);
  }

  /**
   * @param {string} headersText
   * @returns {{ headers: Headers, disposition: Map<string, string> }}
   */
  #parseHeaders(headersText) {
    const headers = new Headers();
    const rawHeaders = StringPrototypeSplit(headersText, "\r\n");
    let headerCount = 0;
    for (let i = 0; i < rawHeaders.length; ++i) {
      const rawHeader = rawHeaders[i];
      const sepIndex = StringPrototypeIndexOf(rawHeader, ":");
      if (sepIndex < 0) {
        continue; // Skip this header
      }
      headerCount++;
      if (headerCount > MAX_MULTIPART_PART_HEADER_COUNT) {
        throw new TypeError("Multipart part has too many headers");
      }
      const key = StringPrototypeSlice(rawHeader, 0, sepIndex);
      const value = StringPrototypeSlice(rawHeader, sepIndex + 1);
      headers.set(key, value);
    }

    const disposition = parseContentDisposition(
      headers.get("Content-Disposition") ?? "",
    );

    return { headers, disposition };
  }

  /**
   * @param {number} index
   * @returns {boolean}
   */
  #matchesBoundary(index) {
    for (let i = 0; i < this.boundaryChars.length; i++) {
      if (this.body[index + i] !== this.boundaryChars[i]) {
        return false;
      }
    }

    return true;
  }

  /**
   * @param {number} index
   * @returns {{ type: 1 | 2, nextIndex: number } | null}
   */
  #matchDelimiter(index) {
    if (!this.#matchesBoundary(index)) {
      return null;
    }

    let suffixIndex = index + this.boundaryChars.length;
    /** @type {1 | 2} */
    let type = 1;
    if (
      this.body[suffixIndex] === DASH &&
      this.body[suffixIndex + 1] === DASH
    ) {
      type = 2;
      suffixIndex += 2;
    }

    // RFC 2046 section 5.1.1 requires receivers to accept transport
    // padding on every boundary delimiter line.
    while (
      this.body[suffixIndex] === SPACE || this.body[suffixIndex] === TAB
    ) {
      suffixIndex++;
    }

    if (type === 2 && suffixIndex === this.body.length) {
      return { type, nextIndex: suffixIndex };
    }
    if (
      this.body[suffixIndex] === CR && this.body[suffixIndex + 1] === LF
    ) {
      return { type, nextIndex: suffixIndex + 2 };
    }

    return null;
  }

  /**
   * @returns {{ type: 1 | 2, headerStart: number } | null}
   */
  #findInitialDelimiter() {
    // RFC 2046 section 5.1.1 requires receivers to ignore the preamble.
    // Only consider boundary delimiters at the start of a line so that
    // boundary-looking bytes within the preamble are not accepted.
    for (let index = 0; index < this.body.length; index++) {
      const isLineStart = index === 0 ||
        (this.body[index - 2] === CR && this.body[index - 1] === LF);
      if (!isLineStart) {
        continue;
      }

      const delimiter = this.#matchDelimiter(index);
      if (delimiter !== null) {
        return { type: delimiter.type, headerStart: delimiter.nextIndex };
      }
    }

    return null;
  }

  /**
   * @returns {FormData}
   */
  parse() {
    const initialDelimiter = this.#findInitialDelimiter();
    if (initialDelimiter === null) {
      throw new TypeError("Unable to parse body as form data");
    }

    const formData = new FormData();
    if (initialDelimiter.type === 2) {
      return formData;
    }

    let headerText = "";
    let headerStart = initialDelimiter.headerStart;
    let state = 1;
    let fileStart = 0;

    for (let i = headerStart; i < this.body.length; i++) {
      const byte = this.body[i];
      const prevByte = this.body[i - 1];
      const isNewLine = byte === LF && prevByte === CR;

      if (state === 1) {
        const headerByteLength = i - headerStart + 1;
        if (headerByteLength > MAX_MULTIPART_PART_HEADER_SIZE) {
          throw new TypeError("Multipart part headers are too large");
        }
        if (
          isNewLine && this.body[i + 1] === CR &&
          this.body[i + 2] === LF
        ) {
          // end of the headers section
          headerText = decodeLatin1Bytes(
            TypedArrayPrototypeSubarray(this.body, headerStart, i + 1),
          );
          state = 2;
          fileStart = i + 3; // After \r\n
        }
      } else if (state === 2) {
        if (isNewLine) {
          const delimiter = this.#matchDelimiter(i + 1);
          if (delimiter === null) {
            continue;
          }

          const { headers, disposition } = this.#parseHeaders(headerText);
          const content = TypedArrayPrototypeSubarray(
            this.body,
            fileStart,
            i - 1,
          );
          // https://fetch.spec.whatwg.org/#ref-for-dom-body-formdata
          // These are UTF-8 decoded as if it was Latin-1.
          // TODO(@andreubotella): Maybe we shouldn't be parsing entry headers
          // as Latin-1.
          const hasFilename = MapPrototypeHas(disposition, "filename");
          const latin1Filename = MapPrototypeGet(disposition, "filename");
          const latin1Name = MapPrototypeGet(disposition, "name");

          headerText = "";

          // Skip nameless parts, but still advance past the matched delimiter.
          if (latin1Name) {
            const name = decodeLatin1StringAsUtf8(latin1Name);
            if (hasFilename) {
              const blob = new Blob([content], {
                type: headers.get("Content-Type") || "application/octet-stream",
              });
              formData.append(
                name,
                blob,
                decodeLatin1StringAsUtf8(latin1Filename),
              );
            } else {
              formData.append(name, core.decode(content));
            }
          }

          if (delimiter.type === 2) {
            break;
          }

          state = 1;
          headerStart = delimiter.nextIndex;
          i = headerStart - 1;
        }
      }
    }

    return formData;
  }
}

/**
 * @param {Uint8Array} body
 * @param {string | undefined} boundary
 * @returns {FormData}
 */
function parseFormData(body, boundary) {
  const parser = new MultipartParser(body, boundary);
  return parser.parse();
}

/**
 * @param {FormDataEntry[]} entries
 * @returns {FormData}
 */
function formDataFromEntries(entries) {
  const fd = new FormData();
  fd[entryList] = entries;
  return fd;
}

webidl.converters["FormData"] = webidl
  .createInterfaceConverter("FormData", FormDataPrototype);

return {
  FormData,
  formDataFromEntries,
  FormDataPrototype,
  formDataToBlob,
  parseFormData,
};
})();
