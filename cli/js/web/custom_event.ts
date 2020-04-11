// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.d.ts";
import * as event from "./event.ts";
import { requiredArguments } from "./util.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class CustomEvent<T = any> extends event.Event
  implements domTypes.CustomEvent {
  #detail: T;

  constructor(type: string, eventInitDict: domTypes.CustomEventInit<T> = {}) {
    super(type, eventInitDict);
    requiredArguments("CustomEvent", arguments.length, 1);
    const { detail } = eventInitDict;
    this.#detail = detail as T;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get detail(): T {
    return this.#detail;
  }

  get [Symbol.toStringTag](): string {
    return "CustomEvent";
  }
}

Reflect.defineProperty(CustomEvent.prototype, "detail", { enumerable: true });
