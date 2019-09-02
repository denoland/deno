// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { DenoError, ErrorKind } from "./errors.ts";
import { hasOwnProperty, requiredArguments } from "./util.ts";
import {
  getRoot,
  isNode,
  isShadowRoot,
  isShadowInclusiveAncestor,
  isSlotable,
  retarget
} from "./dom_util.ts";

// https://dom.spec.whatwg.org/#get-the-parent
// Note: Nodes, shadow roots, and documents override this algorithm so we set it to null.
function getEventTargetParent(
  _eventTarget: domTypes.EventTarget,
  _event: domTypes.Event
): null {
  return null;
}

export class EventListenerOptions implements domTypes.EventListenerOptions {
  _capture = false;

  constructor({ capture = false } = {}) {
    this._capture = capture;
  }

  get capture(): boolean {
    return this._capture;
  }
}

export class AddEventListenerOptions extends EventListenerOptions
  implements domTypes.AddEventListenerOptions {
  _passive = false;
  _once = false;

  constructor({ capture = false, passive = false, once = false } = {}) {
    super({ capture });
    this._passive = passive;
    this._once = once;
  }

  get passive(): boolean {
    return this._passive;
  }

  get once(): boolean {
    return this._once;
  }
}

export class EventListener implements domTypes.EventListener {
  allEvents: domTypes.Event[] = [];
  atEvents: domTypes.Event[] = [];
  bubbledEvents: domTypes.Event[] = [];
  capturedEvents: domTypes.Event[] = [];

  private _callback: (event: domTypes.Event) => void | null;
  private _options: boolean | domTypes.AddEventListenerOptions = false;

  constructor(
    callback: (event: domTypes.Event) => void | null,
    options: boolean | domTypes.AddEventListenerOptions
  ) {
    this._callback = callback;
    this._options = options;
  }

  public handleEvent(event: domTypes.Event): void {
    this.allEvents.push(event);

    switch (event.eventPhase) {
      case domTypes.EventPhase.CAPTURING_PHASE:
        this.capturedEvents.push(event);
        break;
      case domTypes.EventPhase.AT_TARGET:
        this.atEvents.push(event);
        break;
      case domTypes.EventPhase.BUBBLING_PHASE:
        this.bubbledEvents.push(event);
        break;
      default:
        throw new Error("Unspecified event phase");
    }

    this._callback(event);
  }

  get callback(): (event: domTypes.Event) => void | null {
    return this._callback;
  }

  get options(): domTypes.AddEventListenerOptions | boolean {
    return this._options;
  }
}

export const eventTargetAssignedSlot: unique symbol = Symbol();
export const eventTargetHasActivationBehavior: unique symbol = Symbol();

export class EventTarget implements domTypes.EventTarget {
  public [domTypes.eventTargetHost]: domTypes.EventTarget | null = null;
  public [domTypes.eventTargetListeners]: {
    [type in string]: domTypes.EventListener[]
  } = {};
  public [domTypes.eventTargetMode] = "";
  public [domTypes.eventTargetNodeType]: domTypes.NodeType =
    domTypes.NodeType.DOCUMENT_FRAGMENT_NODE;
  private [eventTargetAssignedSlot] = false;
  private [eventTargetHasActivationBehavior] = false;

  public addEventListener(
    type: string,
    callback: (event: domTypes.Event) => void | null,
    options?: domTypes.AddEventListenerOptions | boolean
  ): void {
    requiredArguments("EventTarget.addEventListener", arguments.length, 2);
    const normalizedOptions: domTypes.AddEventListenerOptions = eventTargetHelpers.normalizeAddEventHandlerOptions(
      options
    );

    if (callback === null) {
      return;
    }

    const listeners = this[domTypes.eventTargetListeners];

    if (!hasOwnProperty(listeners, type)) {
      listeners[type] = [];
    }

    for (let i = 0; i < listeners[type].length; ++i) {
      const listener = listeners[type][i];
      if (
        ((typeof listener.options === "boolean" &&
          listener.options === normalizedOptions.capture) ||
          (typeof listener.options === "object" &&
            listener.options.capture === normalizedOptions.capture)) &&
        listener.callback === callback
      ) {
        return;
      }
    }

    listeners[type].push(new EventListener(callback, normalizedOptions));
  }

  public removeEventListener(
    type: string,
    callback: (event: domTypes.Event) => void | null,
    options?: domTypes.EventListenerOptions | boolean
  ): void {
    requiredArguments("EventTarget.removeEventListener", arguments.length, 2);
    const listeners = this[domTypes.eventTargetListeners];
    if (hasOwnProperty(listeners, type) && callback !== null) {
      listeners[type] = listeners[type].filter(
        (listener): boolean => listener.callback !== callback
      );
    }

    const normalizedOptions: domTypes.EventListenerOptions = eventTargetHelpers.normalizeEventHandlerOptions(
      options
    );

    if (callback === null) {
      // Optimization, not in the spec.
      return;
    }

    if (!listeners[type]) {
      return;
    }

    for (let i = 0; i < listeners[type].length; ++i) {
      const listener = listeners[type][i];

      if (
        ((typeof listener.options === "boolean" &&
          listener.options === normalizedOptions.capture) ||
          (typeof listener.options === "object" &&
            listener.options.capture === normalizedOptions.capture)) &&
        listener.callback === callback
      ) {
        listeners[type].splice(i, 1);
        break;
      }
    }
  }

  public dispatchEvent(event: domTypes.Event): boolean {
    requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
    const listeners = this[domTypes.eventTargetListeners];
    if (!hasOwnProperty(listeners, event.type)) {
      return true;
    }

    if (event.dispatched || !event.initialized) {
      throw new DenoError(
        ErrorKind.InvalidData,
        "Tried to dispatch an uninitialized event"
      );
    }

    if (event.eventPhase !== domTypes.EventPhase.NONE) {
      throw new DenoError(
        ErrorKind.InvalidData,
        "Tried to dispatch a dispatching event"
      );
    }

    return eventTargetHelpers.dispatch(this, event);
  }

  get [Symbol.toStringTag](): string {
    return "EventTarget";
  }
}

const eventTargetHelpers = {
  // https://dom.spec.whatwg.org/#concept-event-dispatch
  dispatch(
    targetImpl: EventTarget,
    eventImpl: domTypes.Event,
    targetOverride?: domTypes.EventTarget
  ): boolean {
    let clearTargets = false;
    let activationTarget = null;

    eventImpl.dispatched = true;

    targetOverride = targetOverride || targetImpl;
    let relatedTarget = retarget(eventImpl.relatedTarget, targetImpl);

    if (
      targetImpl !== relatedTarget ||
      targetImpl === eventImpl.relatedTarget
    ) {
      const touchTargets: domTypes.EventTarget[] = [];

      eventTargetHelpers.appendToEventPath(
        eventImpl,
        targetImpl,
        targetOverride,
        relatedTarget,
        touchTargets,
        false
      );

      const isActivationEvent = eventImpl.type === "click";

      if (isActivationEvent && targetImpl[eventTargetHasActivationBehavior]) {
        activationTarget = targetImpl;
      }

      let slotInClosedTree = false;
      let slotable =
        isSlotable(targetImpl) && targetImpl[eventTargetAssignedSlot]
          ? targetImpl
          : null;
      let parent = getEventTargetParent(targetImpl, eventImpl);

      // Populate event path
      // https://dom.spec.whatwg.org/#event-path
      while (parent !== null) {
        if (slotable !== null) {
          slotable = null;

          const parentRoot = getRoot(parent);
          if (
            isShadowRoot(parentRoot) &&
            parentRoot &&
            parentRoot[domTypes.eventTargetMode] === "closed"
          ) {
            slotInClosedTree = true;
          }
        }

        relatedTarget = retarget(eventImpl.relatedTarget, parent);

        if (
          isNode(parent) &&
          isShadowInclusiveAncestor(getRoot(targetImpl), parent)
        ) {
          eventTargetHelpers.appendToEventPath(
            eventImpl,
            parent,
            null,
            relatedTarget,
            touchTargets,
            slotInClosedTree
          );
        } else if (parent === relatedTarget) {
          parent = null;
        } else {
          targetImpl = parent;

          if (
            isActivationEvent &&
            activationTarget === null &&
            targetImpl[eventTargetHasActivationBehavior]
          ) {
            activationTarget = targetImpl;
          }

          eventTargetHelpers.appendToEventPath(
            eventImpl,
            parent,
            targetImpl,
            relatedTarget,
            touchTargets,
            slotInClosedTree
          );
        }

        if (parent !== null) {
          parent = getEventTargetParent(parent, eventImpl);
        }

        slotInClosedTree = false;
      }

      let clearTargetsTupleIndex = -1;
      for (
        let i = eventImpl.path.length - 1;
        i >= 0 && clearTargetsTupleIndex === -1;
        i--
      ) {
        if (eventImpl.path[i].target !== null) {
          clearTargetsTupleIndex = i;
        }
      }
      const clearTargetsTuple = eventImpl.path[clearTargetsTupleIndex];

      clearTargets =
        (isNode(clearTargetsTuple.target) &&
          isShadowRoot(getRoot(clearTargetsTuple.target))) ||
        (isNode(clearTargetsTuple.relatedTarget) &&
          isShadowRoot(getRoot(clearTargetsTuple.relatedTarget)));

      eventImpl.eventPhase = domTypes.EventPhase.CAPTURING_PHASE;

      for (let i = eventImpl.path.length - 1; i >= 0; --i) {
        const tuple = eventImpl.path[i];

        if (tuple.target === null) {
          eventTargetHelpers.invokeEventListeners(targetImpl, tuple, eventImpl);
        }
      }

      for (let i = 0; i < eventImpl.path.length; i++) {
        const tuple = eventImpl.path[i];

        if (tuple.target !== null) {
          eventImpl.eventPhase = domTypes.EventPhase.AT_TARGET;
        } else {
          eventImpl.eventPhase = domTypes.EventPhase.BUBBLING_PHASE;
        }

        if (
          (eventImpl.eventPhase === domTypes.EventPhase.BUBBLING_PHASE &&
            eventImpl.bubbles) ||
          eventImpl.eventPhase === domTypes.EventPhase.AT_TARGET
        ) {
          eventTargetHelpers.invokeEventListeners(targetImpl, tuple, eventImpl);
        }
      }
    }

    eventImpl.eventPhase = domTypes.EventPhase.NONE;

    eventImpl.currentTarget = null;
    eventImpl.path = [];
    eventImpl.dispatched = false;
    eventImpl.cancelBubble = false;
    eventImpl.cancelBubbleImmediately = false;

    if (clearTargets) {
      eventImpl.target = null;
      eventImpl.relatedTarget = null;
    }

    // TODO: invoke activation targets if HTML nodes will be implemented
    // if (activationTarget !== null) {
    //   if (!eventImpl.defaultPrevented) {
    //     activationTarget._activationBehavior();
    //   }
    // }

    return !eventImpl.defaultPrevented;
  },

  // https://dom.spec.whatwg.org/#concept-event-listener-invoke
  invokeEventListeners(
    targetImpl: EventTarget,
    tuple: domTypes.EventPath,
    eventImpl: domTypes.Event
  ): void {
    const tupleIndex = eventImpl.path.indexOf(tuple);
    for (let i = tupleIndex; i >= 0; i--) {
      const t = eventImpl.path[i];
      if (t.target) {
        eventImpl.target = t.target;
        break;
      }
    }

    eventImpl.relatedTarget = tuple.relatedTarget;

    if (eventImpl.cancelBubble) {
      return;
    }

    eventImpl.currentTarget = tuple.item;

    eventTargetHelpers.innerInvokeEventListeners(
      targetImpl,
      eventImpl,
      tuple.item[domTypes.eventTargetListeners]
    );
  },

  // https://dom.spec.whatwg.org/#concept-event-listener-inner-invoke
  innerInvokeEventListeners(
    targetImpl: EventTarget,
    eventImpl: domTypes.Event,
    targetListeners: { [type in string]: domTypes.EventListener[] }
  ): boolean {
    let found = false;

    const { type } = eventImpl;

    if (!targetListeners || !targetListeners[type]) {
      return found;
    }

    // Copy event listeners before iterating since the list can be modified during the iteration.
    const handlers = targetListeners[type].slice();

    for (let i = 0; i < handlers.length; i++) {
      const listener = handlers[i];

      let capture, once, passive;
      if (typeof listener.options === "boolean") {
        capture = listener.options;
        once = false;
        passive = false;
      } else {
        capture = listener.options.capture;
        once = listener.options.once;
        passive = listener.options.passive;
      }

      // Check if the event listener has been removed since the listeners has been cloned.
      if (!targetListeners[type].includes(listener)) {
        continue;
      }

      found = true;

      if (
        (eventImpl.eventPhase === domTypes.EventPhase.CAPTURING_PHASE &&
          !capture) ||
        (eventImpl.eventPhase === domTypes.EventPhase.BUBBLING_PHASE && capture)
      ) {
        continue;
      }

      if (once) {
        targetListeners[type].splice(
          targetListeners[type].indexOf(listener),
          1
        );
      }

      if (passive) {
        eventImpl.inPassiveListener = true;
      }

      try {
        if (listener.callback && typeof listener.handleEvent === "function") {
          listener.handleEvent(eventImpl);
        }
      } catch (error) {
        throw new DenoError(ErrorKind.Interrupted, error.message);
      }

      eventImpl.inPassiveListener = false;

      if (eventImpl.cancelBubbleImmediately) {
        return found;
      }
    }

    return found;
  },

  normalizeAddEventHandlerOptions(
    options: boolean | domTypes.AddEventListenerOptions | undefined
  ): domTypes.AddEventListenerOptions {
    if (typeof options === "boolean" || typeof options === "undefined") {
      const returnValue: domTypes.AddEventListenerOptions = {
        capture: Boolean(options),
        once: false,
        passive: false
      };

      return returnValue;
    } else {
      return options;
    }
  },

  normalizeEventHandlerOptions(
    options: boolean | domTypes.EventListenerOptions | undefined
  ): domTypes.EventListenerOptions {
    if (typeof options === "boolean" || typeof options === "undefined") {
      const returnValue: domTypes.EventListenerOptions = {
        capture: Boolean(options)
      };

      return returnValue;
    } else {
      return options;
    }
  },

  // https://dom.spec.whatwg.org/#concept-event-path-append
  appendToEventPath(
    eventImpl: domTypes.Event,
    target: domTypes.EventTarget,
    targetOverride: domTypes.EventTarget | null,
    relatedTarget: domTypes.EventTarget | null,
    touchTargets: domTypes.EventTarget[],
    slotInClosedTree: boolean
  ): void {
    const itemInShadowTree = isNode(target) && isShadowRoot(getRoot(target));
    const rootOfClosedTree =
      isShadowRoot(target) && target[domTypes.eventTargetMode] === "closed";

    eventImpl.path.push({
      item: target,
      itemInShadowTree,
      target: targetOverride,
      relatedTarget,
      touchTargetList: touchTargets,
      rootOfClosedTree,
      slotInClosedTree
    });
  }
};

/** Built-in objects providing `get` methods for our
 * interceptable JavaScript operations.
 */
Reflect.defineProperty(EventTarget.prototype, "addEventListener", {
  enumerable: true
});
Reflect.defineProperty(EventTarget.prototype, "removeEventListener", {
  enumerable: true
});
Reflect.defineProperty(EventTarget.prototype, "dispatchEvent", {
  enumerable: true
});
