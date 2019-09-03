// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import * as event from "./event.ts";
import { getPrivateValue, requiredArguments } from "./util.ts";

// WeakMaps are recommended for private attributes (see MDN link below)
// https://developer.mozilla.org/en-US/docs/Archive/Add-ons/Add-on_SDK/Guides/Contributor_s_Guide/Private_Properties#Using_WeakMaps
export const customEventAttributes = new WeakMap();

export class CustomEventInit extends event.EventInit
  implements domTypes.CustomEventInit {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  detail: any;

  constructor({
    bubbles = false,
    cancelable = false,
    composed = false,
    detail = null
  }: domTypes.CustomEventInit) {
    super({ bubbles, cancelable, composed });
    this.detail = detail;
  }
}

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

/** Built-in objects providing `get` methods for our
 * interceptable JavaScript operations.
 */
Reflect.defineProperty(CustomEvent.prototype, "detail", { enumerable: true });
