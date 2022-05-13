// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const {
    ArrayPrototypeSlice,
    Error,
    ErrorPrototype,
    ObjectDefineProperty,
    ObjectEntries,
    ObjectPrototypeIsPrototypeOf,
    ObjectSetPrototypeOf,
    SymbolFor,
  } = window.__bootstrap.primordials;
  const webidl = window.__bootstrap.webidl;
  const consoleInternal = window.__bootstrap.console;

  // Defined in WebIDL 4.3.
  // https://heycam.github.io/webidl/#idl-DOMException
  const INDEX_SIZE_ERR = 1;
  const DOMSTRING_SIZE_ERR = 2;
  const HIERARCHY_REQUEST_ERR = 3;
  const WRONG_DOCUMENT_ERR = 4;
  const INVALID_CHARACTER_ERR = 5;
  const NO_DATA_ALLOWED_ERR = 6;
  const NO_MODIFICATION_ALLOWED_ERR = 7;
  const NOT_FOUND_ERR = 8;
  const NOT_SUPPORTED_ERR = 9;
  const INUSE_ATTRIBUTE_ERR = 10;
  const INVALID_STATE_ERR = 11;
  const SYNTAX_ERR = 12;
  const INVALID_MODIFICATION_ERR = 13;
  const NAMESPACE_ERR = 14;
  const INVALID_ACCESS_ERR = 15;
  const VALIDATION_ERR = 16;
  const TYPE_MISMATCH_ERR = 17;
  const SECURITY_ERR = 18;
  const NETWORK_ERR = 19;
  const ABORT_ERR = 20;
  const URL_MISMATCH_ERR = 21;
  const QUOTA_EXCEEDED_ERR = 22;
  const TIMEOUT_ERR = 23;
  const INVALID_NODE_TYPE_ERR = 24;
  const DATA_CLONE_ERR = 25;

  // Defined in WebIDL 2.8.1.
  // https://heycam.github.io/webidl/#dfn-error-names-table
  /** @type {Record<string, number>} */
  const nameToCodeMapping = {
    IndexSizeError: INDEX_SIZE_ERR,
    HierarchyRequestError: HIERARCHY_REQUEST_ERR,
    WrongDocumentError: WRONG_DOCUMENT_ERR,
    InvalidCharacterError: INVALID_CHARACTER_ERR,
    NoModificationAllowedError: NO_MODIFICATION_ALLOWED_ERR,
    NotFoundError: NOT_FOUND_ERR,
    NotSupportedError: NOT_SUPPORTED_ERR,
    InUseAttributeError: INUSE_ATTRIBUTE_ERR,
    InvalidStateError: INVALID_STATE_ERR,
    SyntaxError: SYNTAX_ERR,
    InvalidModificationError: INVALID_MODIFICATION_ERR,
    NamespaceError: NAMESPACE_ERR,
    InvalidAccessError: INVALID_ACCESS_ERR,
    TypeMismatchError: TYPE_MISMATCH_ERR,
    SecurityError: SECURITY_ERR,
    NetworkError: NETWORK_ERR,
    AbortError: ABORT_ERR,
    URLMismatchError: URL_MISMATCH_ERR,
    QuotaExceededError: QUOTA_EXCEEDED_ERR,
    TimeoutError: TIMEOUT_ERR,
    InvalidNodeTypeError: INVALID_NODE_TYPE_ERR,
    DataCloneError: DATA_CLONE_ERR,
  };

  // Defined in WebIDL 4.3.
  // https://heycam.github.io/webidl/#idl-DOMException
  class DOMException {
    #message = "";
    #name = "";
    #code = 0;

    constructor(message = "", name = "Error") {
      this.#message = webidl.converters.DOMString(message, {
        prefix: "Failed to construct 'DOMException'",
        context: "Argument 1",
      });
      this.#name = webidl.converters.DOMString(name, {
        prefix: "Failed to construct 'DOMException'",
        context: "Argument 2",
      });
      this.#code = nameToCodeMapping[this.#name] ?? 0;

      const error = new Error(this.#message);
      error.name = "DOMException";
      ObjectDefineProperty(this, "stack", {
        value: error.stack,
        writable: true,
        configurable: true,
      });

      // `DOMException` isn't a native error, so `Error.prepareStackTrace()` is
      // not called when accessing `.stack`, meaning our structured stack trace
      // hack doesn't apply. This patches it in.
      ObjectDefineProperty(this, "__callSiteEvals", {
        value: ArrayPrototypeSlice(error.__callSiteEvals, 1),
        configurable: true,
      });
    }

    get message() {
      return this.#message;
    }

    get name() {
      return this.#name;
    }

    get code() {
      return this.#code;
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      if (ObjectPrototypeIsPrototypeOf(DOMExceptionPrototype, this)) {
        return `DOMException: ${this.#message}`;
      } else {
        return inspect(consoleInternal.createFilteredInspectProxy({
          object: this,
          evaluate: false,
          keys: [
            "message",
            "name",
            "code",
          ],
        }));
      }
    }
  }

  ObjectSetPrototypeOf(DOMException.prototype, ErrorPrototype);

  webidl.configurePrototype(DOMException);
  const DOMExceptionPrototype = DOMException.prototype;

  for (
    const [key, value] of ObjectEntries({
      INDEX_SIZE_ERR,
      DOMSTRING_SIZE_ERR,
      HIERARCHY_REQUEST_ERR,
      WRONG_DOCUMENT_ERR,
      INVALID_CHARACTER_ERR,
      NO_DATA_ALLOWED_ERR,
      NO_MODIFICATION_ALLOWED_ERR,
      NOT_FOUND_ERR,
      NOT_SUPPORTED_ERR,
      INUSE_ATTRIBUTE_ERR,
      INVALID_STATE_ERR,
      SYNTAX_ERR,
      INVALID_MODIFICATION_ERR,
      NAMESPACE_ERR,
      INVALID_ACCESS_ERR,
      VALIDATION_ERR,
      TYPE_MISMATCH_ERR,
      SECURITY_ERR,
      NETWORK_ERR,
      ABORT_ERR,
      URL_MISMATCH_ERR,
      QUOTA_EXCEEDED_ERR,
      TIMEOUT_ERR,
      INVALID_NODE_TYPE_ERR,
      DATA_CLONE_ERR,
    })
  ) {
    const desc = { value, enumerable: true };
    ObjectDefineProperty(DOMException, key, desc);
    ObjectDefineProperty(DOMException.prototype, key, desc);
  }

  window.__bootstrap.domException = { DOMException };
})(this);
