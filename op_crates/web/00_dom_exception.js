// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { defineProperty } = Object;
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
      this.#message = String(message);
      this.#name = name;
      this.#code = nameToCodeMapping[name] ?? 0;
      this.stack = new Error().stack.replace(
        /^Error/,
        this.#message ? `DOMException: ${this.#message}` : "DOMException",
      );
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

    get [Symbol.toStringTag]() {
      return "DOMException";
    }
  }

  // According to WPT (DOMException-custom-bindings.any.js),
  // the prototype inherits from Error.prototype, but the class itself doesn't inherit
  // from Error. So we avoid using class...extends, and instead use Object.setPrototypeOf
  // for only inheriting from the prototype.
  Object.setPrototypeOf(DOMException.prototype, Error.prototype);

  defineProperty(DOMException.prototype, "message", { enumerable: true });
  defineProperty(DOMException.prototype, "name", { enumerable: true });
  defineProperty(DOMException.prototype, "code", { enumerable: true });

  for (
    const [key, value] of Object.entries({
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
    defineProperty(DOMException, key, desc);
    defineProperty(DOMException.prototype, key, desc);
  }

  window.DOMException = DOMException;
  defineProperty(window, "DOMException", { enumerable: false });
})(this);
