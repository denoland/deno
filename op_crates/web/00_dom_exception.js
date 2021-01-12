// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const nameToCodeMapping = Object.create(
    null,
    {
      IndexSizeError: { value: 1 },
      HierarchyRequestError: { value: 3 },
      WrongDocumentError: { value: 4 },
      InvalidCharacterError: { value: 5 },
      NoModificationAllowedError: { value: 7 },
      NotFoundError: { value: 8 },
      NotSupportedError: { value: 9 },
      InvalidStateError: { value: 11 },
      SyntaxError: { value: 12 },
      InvalidModificationError: { value: 13 },
      NamespaceError: { value: 14 },
      InvalidAccessError: { value: 15 },
      TypeMismatchError: { value: 17 },
      SecurityError: { value: 18 },
      NetworkError: { value: 19 },
      AbortError: { value: 20 },
      URLMismatchError: { value: 21 },
      QuotaExceededError: { value: 22 },
      TimeoutError: { value: 23 },
      InvalidNodeTypeError: { value: 24 },
      DataCloneError: { value: 25 },
    },
  );
  class DOMException extends Error {
    #name = "";
    #code = 0;

    constructor(message = "", name = "Error") {
      super(message);
      this.#name = name;
      this.#code = nameToCodeMapping[name] ?? 0;
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
