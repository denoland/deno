// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const nameToCodeMapping = new Map(
    Object.entries({
      IndexSizeError: 1,
      HierarchyRequestError: 3,
      WrongDocumentError: 4,
      InvalidCharacterError: 5,
      NoModificationAllowedError: 7,
      NotFoundError: 8,
      NotSupportedError: 9,
      InvalidStateError: 11,
      SyntaxError: 12,
      InvalidModificationError: 13,
      NamespaceError: 14,
      InvalidAccessError: 15,
      TypeMismatchError: 17,
      SecurityError: 18,
      NetworkError: 19,
      AbortError: 20,
      URLMismatchError: 21,
      QuotaExceededError: 22,
      TimeoutError: 23,
      InvalidNodeTypeError: 24,
      DataCloneError: 25,
    }),
  );

  class DOMException extends Error {
    #name = "";
    #code = 0;

    constructor(message = "", name = "Error") {
      super(message);
      this.#name = name;
      this.#code = nameToCodeMapping.get(name) ?? 0;
    }

    get name() {
      return this.#name;
    }

    get code() {
      return this.#code;
    }
  }

  window.DOMException = DOMException;
})(this);
