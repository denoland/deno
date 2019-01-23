// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import * as event from "./event";
import { getPrivateValue } from "./util";

// WeakMaps are recommended for private attributes (see MDN link below)
// tslint:disable-next-line:max-line-length
// https://developer.mozilla.org/en-US/docs/Archive/Add-ons/Add-on_SDK/Guides/Contributor_s_Guide/Private_Properties#Using_WeakMaps
export const customEventAttributes = new WeakMap();

export class CustomEventInit extends event.EventInit
  implements domTypes.CustomEventInit {
  // tslint:disable-next-line:no-any
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
    super(type, customEventInitDict);
    const { detail = null } = customEventInitDict;
    customEventAttributes.set(this, { detail });
  }

  // tslint:disable-next-line:no-any
  get detail(): any {
    return getPrivateValue(this, customEventAttributes, "detail");
  }

  initCustomEvent(
    type: string,
    bubbles?: boolean,
    cancelable?: boolean,
    // tslint:disable-next-line:no-any
    detail?: any
  ) {
    if (this.dispatched) {
      return;
    }

    customEventAttributes.set(this, { detail });
  }
}

/** Built-in objects providing `get` methods for our
 * interceptable JavaScript operations.
 */
Reflect.defineProperty(CustomEvent.prototype, "detail", { enumerable: true });
