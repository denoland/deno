// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { getPrivateValue } from "./util";

// WeakMaps are recommended for private attributes (see MDN link below)
// tslint:disable-next-line:max-line-length
// https://developer.mozilla.org/en-US/docs/Archive/Add-ons/Add-on_SDK/Guides/Contributor_s_Guide/Private_Properties#Using_WeakMaps
export const eventAttributes = new WeakMap();

export class EventInit implements domTypes.EventInit {
  bubbles = false;
  cancelable = false;
  composed = false;

  constructor({ bubbles = false, cancelable = false, composed = false } = {}) {
    this.bubbles = bubbles;
    this.cancelable = cancelable;
    this.composed = composed;
  }
}

export class Event implements domTypes.Event {
  // Each event has the following associated flags
  private _canceledFlag = false;
  private _inPassiveListenerFlag = false;
  private _stopImmediatePropagationFlag = false;
  private _stopPropagationFlag = false;

  // Property for objects on which listeners will be invoked
  private _path: domTypes.EventPath[] = [];

  constructor(type: string, eventInitDict: domTypes.EventInit = {}) {
    eventAttributes.set(this, {
      type,
      bubbles: eventInitDict.bubbles || false,
      cancelable: eventInitDict.cancelable || false,
      composed: eventInitDict.composed || false,
      currentTarget: null,
      eventPhase: domTypes.EventPhase.NONE,
      isTrusted: false,
      target: null,
      timeStamp: Date.now()
    });
  }

  get bubbles(): boolean {
    return getPrivateValue(this, eventAttributes, "bubbles");
  }

  get cancelBubble(): boolean {
    return this._stopPropagationFlag;
  }

  get cancelBubbleImmediately(): boolean {
    return this._stopImmediatePropagationFlag;
  }

  get cancelable(): boolean {
    return getPrivateValue(this, eventAttributes, "cancelable");
  }

  get composed(): boolean {
    return getPrivateValue(this, eventAttributes, "composed");
  }

  get currentTarget(): domTypes.EventTarget {
    return getPrivateValue(this, eventAttributes, "currentTarget");
  }

  get defaultPrevented(): boolean {
    return this._canceledFlag;
  }

  get eventPhase(): number {
    return getPrivateValue(this, eventAttributes, "eventPhase");
  }

  get isTrusted(): boolean {
    return getPrivateValue(this, eventAttributes, "isTrusted");
  }

  get target(): domTypes.EventTarget {
    return getPrivateValue(this, eventAttributes, "target");
  }

  get timeStamp(): Date {
    return getPrivateValue(this, eventAttributes, "timeStamp");
  }

  get type(): string {
    return getPrivateValue(this, eventAttributes, "type");
  }

  /** Returns the event’s path (objects on which listeners will be
   * invoked). This does not include nodes in shadow trees if the
   * shadow root was created with its ShadowRoot.mode closed.
   *
   *      event.composedPath();
   */
  composedPath(): domTypes.EventPath[] {
    if (this._path.length === 0) {
      return [];
    }

    const composedPath: domTypes.EventPath[] = [
      {
        item: this.currentTarget,
        itemInShadowTree: false,
        relatedTarget: null,
        rootOfClosedTree: false,
        slotInClosedTree: false,
        target: null,
        touchTargetList: []
      }
    ];

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
        composedPath.unshift({
          item,
          itemInShadowTree: false,
          relatedTarget: null,
          rootOfClosedTree: false,
          slotInClosedTree: false,
          target: null,
          touchTargetList: []
        });
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
      let index = currentTargetIndex + 1;
      index < this._path.length;
      index++
    ) {
      const { item, rootOfClosedTree, slotInClosedTree } = this._path[index];

      if (slotInClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        composedPath.push({
          item,
          itemInShadowTree: false,
          relatedTarget: null,
          rootOfClosedTree: false,
          slotInClosedTree: false,
          target: null,
          touchTargetList: []
        });
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
Reflect.defineProperty(Event.prototype, "defaultPrevented", {
  enumerable: true
});
Reflect.defineProperty(Event.prototype, "eventPhase", { enumerable: true });
Reflect.defineProperty(Event.prototype, "isTrusted", { enumerable: true });
Reflect.defineProperty(Event.prototype, "target", { enumerable: true });
Reflect.defineProperty(Event.prototype, "timeStamp", { enumerable: true });
Reflect.defineProperty(Event.prototype, "type", { enumerable: true });
