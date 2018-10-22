// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";

export class Event {
  /** Each event has the following associated flags
   * that are all initially unset.
   */
  stopPropagationFlag: boolean;
  stopImmediatePropagationFlag: boolean;
  canceledFlag: boolean;
  inPassiveListenerFlag: boolean = false;
  composedFlag: boolean = false;
  initialized: boolean;
  dispatch: boolean = false;

  readonly type: string = "";
  readonly target: domTypes.EventTarget | null;
  readonly srcElement: domTypes.EventTarget | null = this.target;
  readonly currentTarget: domTypes.EventTarget | null;

  constructor(type: string, eventInitDict?: EventInit) {
    this.initialized = true;
    this.stopPropagationFlag = false;
    this.stopImmediatePropagationFlag = false;
    this.canceledFlag = false;
    this.isTrusted = false;
    this.target = null;
    this.currentTarget = null;
    this.type = type;
    this.bubbles = eventInitDict && eventInitDict.bubbles || false;
    this.cancelable = eventInitDict && eventInitDict.cancelable || false;
  }

  /** Returns the event’s path (objects on which listeners will be
   * invoked). This does not include nodes in shadow trees if the
   * shadow root was created with its ShadowRoot.mode closed.
   *
   *    event.composedPath();
   */
  composedPath(): domTypes.EventTarget[] {
    let reversedComposedPath = [];
    let hiddenSubtreeLevel = 0;
    let hasSeenCurrentTarget = false;
    let currentTarget = this.currentTarget;
    let reversedPath = [];

    reversedPath.forEach(struct => {
      if (struct.item === currentTarget) {
        hasSeenCurrentTarget = true;
      } else if (hasSeenCurrentTarget && struct.rootOfClosedTree) {
        hiddenSubtreeLevel += 1;
      }

      if (hiddenSubtreeLevel === 0) {
        reversedComposedPath.push(struct.item);
      }

      if (struct.slotInClosedTree && hiddenSubtreeLevel > 0) {
        hiddenSubtreeLevel += 1;
      }
    });

    return reversedComposedPath;
  }

  readonly NONE: number = 0;
  readonly CAPTURING_PHASE: number = 1;
  readonly AT_TARGET: number = 2;
  readonly BUBBLING_PHASE: number = 3;
  readonly eventPhase: number = this.NONE;

  /** Stops the propagation of events further along in the DOM.
   *
   *    event.stopPropagation();
   */
  stopPropagation(): void {
    this.stopPropagationFlag = true;
  }

  cancelBubble: boolean = this.stopPropagationFlag;

  /** For this particular event, no other listener will be called.
   * Neither those attached on the same element, nor those attached
   * on elements which will be traversed later (in capture phase,
   * for instance).
   *
   *    event.stopImmediatePropagation();
   */
  stopImmediatePropagation(): void {
    this.stopPropagationFlag = true;
    this.stopImmediatePropagationFlag = true;
  }

  readonly bubbles: boolean;
  readonly cancelable: boolean;
  returnValue: boolean = this.canceledFlag;

  /** Cancels the event (if it is cancelable).
   *
   *    event.preventDefault();
   */
  preventDefault(): void {
    this.canceledFlag = true;
  }

  readonly defaultPrevented: boolean = this.canceledFlag;
  readonly composed: boolean = this.composedFlag;

  readonly isTrusted: boolean;
  readonly timeStamp: number = 0;

  /** Initializes the value of an Event created. If the event has
   * already being dispatched, this method does nothing.
   *
   *    event.initEvent('type', bubbles, cancelable);
   */
  initEvent(type: string, bubbles?: boolean, cancelable?: boolean): void {
    if (this.dispatch) {
      return;
    }

    new Event(type, new EventInit(bubbles, cancelable));
  }
}

export class EventInit {
  bubbles?: boolean = false;
  cancelable?: boolean = false;
  composed?: boolean = false;

  constructor(bubbles?: boolean, cancelable?: boolean, composed?: boolean) {
    this.bubbles = bubbles;
    this.cancelable = cancelable;
    this.composed = composed;
  }
}
