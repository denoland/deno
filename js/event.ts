// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";

export const eventAttributes = new WeakMap();

export class EventInit implements domTypes.EventInit {
  bubbles = false;
  cancelable = false;
  composed = false;

  constructor({bubbles=false, cancelable=false, composed=false} = {}) {
    this.bubbles = bubbles;
    this.cancelable = cancelable;
    this.composed = composed;
  }
}

export class Event implements domTypes.Event {
  // Each event has the following associated flags
  private _stopPropagationFlag = false;
  private _stopImmediatePropagationFlag = false;
  private _canceledFlag = false;
  private _inPassiveListenerFlag = false;
  private _composedFlag = false;
  private _initializedFlag = true;
  private _dispatchFlag = false;

  // Property for objects on which listeners will be invoked
  private _path: domTypes.EventTarget[] = [];

  constructor(type: string, eventInitDict?: domTypes.EventInit) {
    eventAttributes.set(this, {
      type,
      bubbles: eventInitDict && eventInitDict.bubbles || false,
      cancelable: eventInitDict && eventInitDict.cancelable || false,
      composed: eventInitDict && eventInitDict.composed || false,
      currentTarget: null,
      eventPhase: domTypes.EventPhase.NONE,
      isTrusted: false,
      target: null,
      timeStamp: Date.now(),
    });
  }

  get bubbles(): boolean {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).bubbles || false;
    }

    throw new TypeError("Illegal invocation");
  }

  get cancelable(): boolean {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).cancelable || false;
    }

    throw new TypeError("Illegal invocation");
  }

  get composed(): boolean {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).composed || false;
    }

    throw new TypeError("Illegal invocation");
  }

  get currentTarget(): domTypes.EventTarget {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).currentTarget || null;
    }

    throw new TypeError("Illegal invocation");
  }

  get defaultPrevented(): boolean {
    return this._canceledFlag;
  }

  get eventPhase(): number {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).eventPhase || domTypes.EventPhase.NONE;
    }

    throw new TypeError("Illegal invocation");
  }

  get isTrusted(): boolean {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).isTrusted || false;
    }

    throw new TypeError("Illegal invocation");
  }

  get target(): domTypes.EventTarget {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).target || null;
    }

    throw new TypeError("Illegal invocation");
  }

  get timeStamp(): Date {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).timeStamp || Date.now();
    }

    throw new TypeError("Illegal invocation");
  }

  get type(): string {
    if (eventAttributes.has(this)) {
      return eventAttributes.get(this).type || "";
    }

    throw new TypeError("Illegal invocation");
  }

  /** Returns the event’s path (objects on which listeners will be
   * invoked). This does not include nodes in shadow trees if the
   * shadow root was created with its ShadowRoot.mode closed.
   *
   *      event.composedPath();
   */
  composedPath(): domTypes.EventTarget[] {
    const composedPath = [];

    if (this._path.length === 0) {
      return composedPath;
    }

    composedPath.push(this.currentTarget);

    let currentTargetIndex = 0;
    let currentTargetHiddenSubtreeLevel = 0;

    for (let index = this._path.length - 1; index >= 0; index--) {
      const { item, rootOfClosedTree, slotInClosedTree } = this._path[index];

      if (rootOfClosedTree) {
        currentTargetHiddenSubtreeLevel++;
      }

      if (item === this.currentTarget) {
        currentTargetIndex = index;
        break;
      }

      if (slotInClosedTree) {
        currentTargetHiddenSubtreeLevel--;
      }
    }

    let currentHiddenLevel = currentTargetHiddenSubtreeLevel;
    let maxHiddenLevel = currentTargetHiddenSubtreeLevel;

    for (let i = currentTargetIndex - 1; i >= 0; i--) {
      const { item, rootOfClosedTree, slotInClosedTree } = this._path[i];

      if (rootOfClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        composedPath.unshift(item);
      }

      if (slotInClosedTree) {
        currentHiddenLevel--;

        if (currentHiddenLevel < maxHiddenLevel) {
          maxHiddenLevel = currentHiddenLevel;
        }
      }
    }

    currentHiddenLevel = currentTargetHiddenSubtreeLevel;
    maxHiddenLevel = currentTargetHiddenSubtreeLevel;

    for (
      let index = currentTargetIndex + 1; index < this._path.length; index++
     ) {
      const { item, rootOfClosedTree, slotInClosedTree } = this._path[index];

      if (slotInClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        composedPath.push(item);
      }

      if (rootOfClosedTree) {
        currentHiddenLevel--;

        if (currentHiddenLevel < maxHiddenLevel) {
          maxHiddenLevel = currentHiddenLevel;
        }
      }
    }

    return composedPath;
  }

  /** Cancels the event (if it is cancelable).
   * See https://dom.spec.whatwg.org/#set-the-canceled-flag
   *
   *      event.preventDefault();
   */
  preventDefault(): void {
    if (this.cancelable && !this._inPassiveListenerFlag) {
      this._canceledFlag = true;
    }
  }

  /** Stops the propagation of events further along in the DOM.
   *
   *      event.stopPropagation();
   */
  stopPropagation(): void {
    this._stopPropagationFlag = true;
  }

  /** For this particular event, no other listener will be called.
   * Neither those attached on the same element, nor those attached
   * on elements which will be traversed later (in capture phase,
   * for instance).
   *
   *      event.stopImmediatePropagation();
   */
  stopImmediatePropagation(): void {
    this._stopPropagationFlag = true;
    this._stopImmediatePropagationFlag = true;
  }
}

/** Built-in objects providing `get` methods for our
 * interceptable JavaScript operations.
 */
Reflect.defineProperty(Event.prototype, "bubbles", { enumerable: true });
Reflect.defineProperty(Event.prototype, "cancelable", { enumerable: true });
Reflect.defineProperty(Event.prototype, "composed", { enumerable: true });
Reflect.defineProperty(Event.prototype, "currentTarget", { enumerable: true });
Reflect.defineProperty(Event.prototype, "defaultPrevented", { enumerable: true });
Reflect.defineProperty(Event.prototype, "eventPhase", { enumerable: true });
Reflect.defineProperty(Event.prototype, "isTrusted", { enumerable: true });
Reflect.defineProperty(Event.prototype, "target", { enumerable: true });
Reflect.defineProperty(Event.prototype, "timeStamp", { enumerable: true });
Reflect.defineProperty(Event.prototype, "type", { enumerable: true });
