// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export class DOMExceptionImpl extends Error implements DOMException {
  #name: string;

  constructor(message = "", name = "Error") {
    super(message);
    this.#name = name;
  }

  get name(): string {
    return this.#name;
  }
}
