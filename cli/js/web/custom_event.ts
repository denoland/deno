// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import * as event from "./event.ts";
import { getPrivateValue, requiredArguments } from "./util.ts";

// WeakMaps are recommended for private attributes (see MDN link below)
// https://developer.mozilla.org/en-US/docs/Archive/Add-ons/Add-on_SDK/Guides/Contributor_s_Guide/Private_Properties#Using_WeakMaps
export const customEventAttributes = new WeakMap();

export class CustomEvent extends event.Event implements domTypes.CustomEvent {
  constructor(
    type: string,
    customEventInitDict: domTypes.CustomEventInit = {}
  ) {
    requiredArguments("CustomEvent", arguments.length, 1);
    super(type, customEventInitDict);
    const { detail = null } = customEventInitDict;
    customEventAttributes.set(this, { detail });
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get detail(): any {
    return getPrivateValue(this, customEventAttributes, "detail");
  }

  initCustomEvent(
    type: string,
    bubbles?: boolean,
    cancelable?: boolean,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    detail?: any
  ): void {
    if (this.dispatched) {
      return;
    }

    customEventAttributes.set(this, { detail });
  }

  get [Symbol.toStringTag](): string {
    return "CustomEvent";
  }
}

Reflect.defineProperty(CustomEvent.prototype, "detail", { enumerable: true });
