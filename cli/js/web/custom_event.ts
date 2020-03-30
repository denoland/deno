// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import * as event from "./event.ts";
import { requiredArguments } from "./util.ts";

export class CustomEvent extends event.Event implements domTypes.CustomEvent {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  #detail: any;

  constructor(
    type: string,
    customEventInitDict: domTypes.CustomEventInit = {}
  ) {
    super(type, customEventInitDict);
    requiredArguments("CustomEvent", arguments.length, 1);
    const { detail = null } = customEventInitDict;
    this.#detail = detail;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get detail(): any {
    return this.#detail;
  }

  initCustomEvent(
    _type: string,
    _bubbles?: boolean,
    _cancelable?: boolean,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    detail?: any
  ): void {
    if (this.dispatched) {
      return;
    }

    this.#detail = detail;
  }

  get [Symbol.toStringTag](): string {
    return "CustomEvent";
  }
}

Reflect.defineProperty(CustomEvent.prototype, "detail", { enumerable: true });
