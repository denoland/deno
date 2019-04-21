// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { requiredArguments, hasOwnProperty } from "./util";

/* TODO: This is an incomplete implementation to provide functionality
 * for Event. A proper spec is still required for a proper Web API.
 */
export class EventTarget implements domTypes.EventTarget {
  public listeners: {
    [type in string]: domTypes.EventListenerOrEventListenerObject[]
  } = {};

  public addEventListener(
    type: string,
    listener: domTypes.EventListenerOrEventListenerObject | null,
    _options?: boolean | domTypes.AddEventListenerOptions
  ): void {
    requiredArguments("EventTarget.addEventListener", arguments.length, 2);
    if (!hasOwnProperty(this.listeners, type)) {
      this.listeners[type] = [];
    }
    if (listener !== null) {
      this.listeners[type].push(listener);
    }
  }

  public removeEventListener(
    type: string,
    callback: domTypes.EventListenerOrEventListenerObject | null,
    _options?: domTypes.EventListenerOptions | boolean
  ): void {
    requiredArguments("EventTarget.removeEventListener", arguments.length, 2);
    if (hasOwnProperty(this.listeners, type) && callback !== null) {
      this.listeners[type] = this.listeners[type].filter(
        listener => listener !== callback
      );
    }
  }

  public dispatchEvent(event: domTypes.Event): boolean {
    requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
    if (!hasOwnProperty(this.listeners, event.type)) {
      return true;
    }
    const stack = this.listeners[event.type].slice();

    for (const stackElement of stack) {
      if ((stackElement as domTypes.EventListenerObject).handleEvent) {
        (stackElement as domTypes.EventListenerObject).handleEvent(event);
      } else {
        (stackElement as domTypes.EventListener).call(this, event);
      }
    }
    return !event.defaultPrevented;
  }

  get [Symbol.toStringTag](): string {
    return "EventTarget";
  }
}
