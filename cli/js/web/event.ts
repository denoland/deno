// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { getPrivateValue, requiredArguments } from "./util.ts";

// WeakMaps are recommended for private attributes (see MDN link below)
// https://developer.mozilla.org/en-US/docs/Archive/Add-ons/Add-on_SDK/Guides/Contributor_s_Guide/Private_Properties#Using_WeakMaps
export const eventAttributes = new WeakMap();

function isTrusted(this: Event): boolean {
  return getPrivateValue(this, eventAttributes, "isTrusted");
}

export class Event implements domTypes.Event {
  // The default value is `false`.
  // Use `defineProperty` to define on each instance, NOT on the prototype.
  isTrusted!: boolean;
  // Each event has the following associated flags
  private _canceledFlag = false;
  private _dispatchedFlag = false;
  private _initializedFlag = false;
  private _inPassiveListenerFlag = false;
  private _stopImmediatePropagationFlag = false;
  private _stopPropagationFlag = false;

  // Property for objects on which listeners will be invoked
  private _path: domTypes.EventPath[] = [];

  constructor(type: string, eventInitDict: domTypes.EventInit = {}) {
    requiredArguments("Event", arguments.length, 1);
    type = String(type);
    this._initializedFlag = true;
    eventAttributes.set(this, {
      type,
      bubbles: eventInitDict.bubbles || false,
      cancelable: eventInitDict.cancelable || false,
      composed: eventInitDict.composed || false,
      currentTarget: null,
      eventPhase: domTypes.EventPhase.NONE,
      isTrusted: false,
      relatedTarget: null,
      target: null,
      timeStamp: Date.now(),
    });
    Reflect.defineProperty(this, "isTrusted", {
      enumerable: true,
      get: isTrusted,
    });
  }

  get bubbles(): boolean {
    return getPrivateValue(this, eventAttributes, "bubbles");
  }

  get cancelBubble(): boolean {
    return this._stopPropagationFlag;
  }

  set cancelBubble(value: boolean) {
    this._stopPropagationFlag = value;
  }

  get cancelBubbleImmediately(): boolean {
    return this._stopImmediatePropagationFlag;
  }

  set cancelBubbleImmediately(value: boolean) {
    this._stopImmediatePropagationFlag = value;
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

  set currentTarget(value: domTypes.EventTarget) {
    eventAttributes.set(this, {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: value,
      eventPhase: this.eventPhase,
      isTrusted: this.isTrusted,
      relatedTarget: this.relatedTarget,
      target: this.target,
      timeStamp: this.timeStamp,
    });
  }

  get defaultPrevented(): boolean {
    return this._canceledFlag;
  }

  get dispatched(): boolean {
    return this._dispatchedFlag;
  }

  set dispatched(value: boolean) {
    this._dispatchedFlag = value;
  }

  get eventPhase(): number {
    return getPrivateValue(this, eventAttributes, "eventPhase");
  }

  set eventPhase(value: number) {
    eventAttributes.set(this, {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: this.currentTarget,
      eventPhase: value,
      isTrusted: this.isTrusted,
      relatedTarget: this.relatedTarget,
      target: this.target,
      timeStamp: this.timeStamp,
    });
  }

  get initialized(): boolean {
    return this._initializedFlag;
  }

  set inPassiveListener(value: boolean) {
    this._inPassiveListenerFlag = value;
  }

  get path(): domTypes.EventPath[] {
    return this._path;
  }

  set path(value: domTypes.EventPath[]) {
    this._path = value;
  }

  get relatedTarget(): domTypes.EventTarget {
    return getPrivateValue(this, eventAttributes, "relatedTarget");
  }

  set relatedTarget(value: domTypes.EventTarget) {
    eventAttributes.set(this, {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: this.currentTarget,
      eventPhase: this.eventPhase,
      isTrusted: this.isTrusted,
      relatedTarget: value,
      target: this.target,
      timeStamp: this.timeStamp,
    });
  }

  get target(): domTypes.EventTarget {
    return getPrivateValue(this, eventAttributes, "target");
  }

  set target(value: domTypes.EventTarget) {
    eventAttributes.set(this, {
      type: this.type,
      bubbles: this.bubbles,
      cancelable: this.cancelable,
      composed: this.composed,
      currentTarget: this.currentTarget,
      eventPhase: this.eventPhase,
      isTrusted: this.isTrusted,
      relatedTarget: this.relatedTarget,
      target: value,
      timeStamp: this.timeStamp,
    });
  }

  get timeStamp(): Date {
    return getPrivateValue(this, eventAttributes, "timeStamp");
  }

  get type(): string {
    return getPrivateValue(this, eventAttributes, "type");
  }

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
        touchTargetList: [],
      },
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
          touchTargetList: [],
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
          touchTargetList: [],
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

  preventDefault(): void {
    if (this.cancelable && !this._inPassiveListenerFlag) {
      this._canceledFlag = true;
    }
  }

  stopPropagation(): void {
    this._stopPropagationFlag = true;
  }

  stopImmediatePropagation(): void {
    this._stopPropagationFlag = true;
    this._stopImmediatePropagationFlag = true;
  }
}

Reflect.defineProperty(Event.prototype, "bubbles", { enumerable: true });
Reflect.defineProperty(Event.prototype, "cancelable", { enumerable: true });
Reflect.defineProperty(Event.prototype, "composed", { enumerable: true });
Reflect.defineProperty(Event.prototype, "currentTarget", { enumerable: true });
Reflect.defineProperty(Event.prototype, "defaultPrevented", {
  enumerable: true,
});
Reflect.defineProperty(Event.prototype, "dispatched", { enumerable: true });
Reflect.defineProperty(Event.prototype, "eventPhase", { enumerable: true });
Reflect.defineProperty(Event.prototype, "target", { enumerable: true });
Reflect.defineProperty(Event.prototype, "timeStamp", { enumerable: true });
Reflect.defineProperty(Event.prototype, "type", { enumerable: true });
