// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { EventImpl as Event } from "./event.ts";
import { defineEnumerableProps } from "./util.ts";

export class ErrorEventImpl extends Event implements ErrorEvent {
  #message: string;
  #filename: string;
  #lineno: number;
  #colno: number;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  #error: any;

  get message(): string {
    return this.#message;
  }
  get filename(): string {
    return this.#filename;
  }
  get lineno(): number {
    return this.#lineno;
  }
  get colno(): number {
    return this.#colno;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get error(): any {
    return this.#error;
  }

  constructor(
    type: string,
    {
      bubbles,
      cancelable,
      composed,
      message = "",
      filename = "",
      lineno = 0,
      colno = 0,
      error = null,
    }: ErrorEventInit = {}
  ) {
    super(type, {
      bubbles: bubbles,
      cancelable: cancelable,
      composed: composed,
    });

    this.#message = message;
    this.#filename = filename;
    this.#lineno = lineno;
    this.#colno = colno;
    this.#error = error;
  }

  get [Symbol.toStringTag](): string {
    return "ErrorEvent";
  }
}

defineEnumerableProps(ErrorEventImpl, [
  "message",
  "filename",
  "lineno",
  "colno",
  "error",
]);
