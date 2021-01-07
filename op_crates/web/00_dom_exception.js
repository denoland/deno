// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const nameToCodeMapping = Object.create(null);
  nameToCodeMatting.IndexSizeError = 1;
  nameToCodeMapping.HierarchyRequestError = 3;
  nameToCodeMapping.WrongDocumentError = 4;
  nameToCodeMapping.InvalidCharacterError = 5;
  nameToCodeMapping.NoModificationAllowedError = 7;
  nameToCodeMapping.NotFoundError = 8;
  nameToCodeMapping.NotSupportedError = 9;
  nameToCodeMapping.InvalidStateError = 11;
  nameToCodeMapping.SyntaxError = 12;
  nameToCodeMapping.InvalidModificationError = 13;
  nameToCodeMapping.NamespaceError = 14;
  nameToCodeMapping.InvalidAccessError = 15;
  nameToCodeMapping.TypeMismatchError = 17;
  nameToCodeMapping.SecurityError = 18;
  nameToCodeMapping.NetworkError = 19;
  nameToCodeMapping.AbortError = 20;
  nameToCodeMapping.URLMismatchError = 21;
  nameToCodeMapping.QuotaExceededError = 22;
  nameToCodeMapping.TimeoutError = 23;
  nameToCodeMapping.InvalidNodeTypeError = 24;
  nameToCodeMapping.DataCloneError = 25;

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
