// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";

const EVENT_ATTRIBUTES = new WeakMap;

export enum EventPhase {
  NONE = 0,
  CAPTURING_PHASE = 1,
  AT_TARGET = 2,
  BUBBLING_PHASE = 3,
}

export class Event {
  // Each event has the following associated flags
  private stopPropagationFlag: boolean = false;
  private stopImmediatePropagationFlag: boolean = false;
  private canceledFlag: boolean = false;
  private inPassiveListenerFlag: boolean = false;
  private composedFlag: boolean = false;
  private initializedFlag: boolean = true;
  private dispatchFlag: boolean = false;

  // Property for objects on which listeners will be invoked
  private path: domTypes.EventTarget[] = [];

  constructor(type: string, {bubbles=false, cancelable=false, composed=false} = {}) {
    EVENT_ATTRIBUTES.set(this, {
      type,
      bubbles,
      cancelable,
      composed,
      currentTarget: null,
      eventPhase: EventPhase.NONE,
      isTrusted: false,
      target: null,
      timeStamp: Date.now(),
    });
  }

  get bubbles(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).bubbles;
    }

    throw new TypeError('Illegal invocation');
  }

  get cancelable(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).cancelable;
    }

    throw new TypeError('Illegal invocation');
  }

  get composed(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).composed;
    }

    throw new TypeError('Illegal invocation');
  }

  get currentTarget(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).currentTarget;
    }

    throw new TypeError('Illegal invocation');
  }

  get defaultPrevented(): boolean {
    return this.canceledFlag;
  }

  get eventPhase(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).eventPhase;
    }

    throw new TypeError('Illegal invocation');
  }

  get isTrusted(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).isTrusted;
    }

    throw new TypeError('Illegal invocation');
  }

  get target(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).target;
    }

    throw new TypeError('Illegal invocation');
  }

  get timeStamp(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).timeStamp;
    }

    throw new TypeError('Illegal invocation');
  }

  get type(): boolean | TypeError {
    if (EVENT_ATTRIBUTES.has(this)) {
      return EVENT_ATTRIBUTES.get(this).type;
    }

    throw new TypeError('Illegal invocation');
  }

  /** Returns the event’s path (objects on which listeners will be
   * invoked). This does not include nodes in shadow trees if the
   * shadow root was created with its ShadowRoot.mode closed.
   *
   *      event.composedPath();
   */
  composedPath(): domTypes.EventTarget[] {
    const composedPath = [];

    if (this.path.length === 0) {
      return composedPath;
    }

    composedPath.push(this.currentTarget);

    let currentTargetIndex = 0;
    let currentTargetHiddenSubtreeLevel = 0;

    for (let index = this.path.length - 1; index >= 0; index--) {
      const { item, rootOfClosedTree, slotInClosedTree } = this.path[index];

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
      const { item, rootOfClosedTree, slotInClosedTree } = this.path[i];

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

    for (let index = currentTargetIndex + 1; index < this.path.length; index++) {
      const { item, rootOfClosedTree, slotInClosedTree } = this.path[index];

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
    if (this.cancelable && !this.inPassiveListenerFlag) {
      this.canceledFlag = true;
    }
  }

  /** Stops the propagation of events further along in the DOM.
   *
   *      event.stopPropagation();
   */
  stopPropagation(): void {
    this.stopPropagationFlag = true;
  }

  /** For this particular event, no other listener will be called.
   * Neither those attached on the same element, nor those attached
   * on elements which will be traversed later (in capture phase,
   * for instance).
   *
   *      event.stopImmediatePropagation();
   */
  stopImmediatePropagation(): void {
    this.stopPropagationFlag = true;
    this.stopImmediatePropagationFlag = true;
  }
}

// Built-in objects providing `get` methods for our interceptable JavaScript operations
Reflect.defineProperty(Event.prototype, 'bubbles', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'cancelable', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'composed', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'currentTarget', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'defaultPrevented', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'eventPhase', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'isTrusted', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'target', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'timeStamp', { enumerable: true });
Reflect.defineProperty(Event.prototype, 'type', { enumerable: true });
