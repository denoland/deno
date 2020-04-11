// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as domTypes from "./dom_types.d.ts";

export class DOMException extends Error implements domTypes.DOMException {
  #name: string;

  constructor(message = "", name = "Error") {
    super(message);
    this.#name = name;
  }

  get name(): string {
    return this.#name;
  }
}
