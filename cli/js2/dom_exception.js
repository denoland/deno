// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

class DOMException extends Error {
  #name = "";

  constructor(message = "", name = "Error") {
    super(message);
    this.#name = name;
  }

  get name() {
    return this.#name;
  }
}

// globalThis.DOMException = DOMException;
