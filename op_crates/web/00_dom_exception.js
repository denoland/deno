// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// eslint-disable-next-line @typescript-eslint/no-unused-vars
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
