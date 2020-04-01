// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { hasOwnProperty, requiredArguments } from "./util.ts";
import {
  getRoot,
  isNode,
  isShadowRoot,
  isShadowInclusiveAncestor,
  isSlotable,
  retarget,
} from "./dom_util.ts";

// https://dom.spec.whatwg.org/#get-the-parent
// Note: Nodes, shadow roots, and documents override this algorithm so we set it to null.
function getEventTargetParent(
  _eventTarget: domTypes.EventTarget,
  _event: domTypes.Event
): null {
  return null;
}

export const eventTargetAssignedSlot: unique symbol = Symbol();
export const eventTargetHasActivationBehavior: unique symbol = Symbol();

export class EventTarget implements domTypes.EventTarget {
  public [domTypes.eventTargetHost]: domTypes.EventTarget | null = null;
  public [domTypes.eventTargetListeners]: {
    [type in string]: domTypes.EventTargetListener[];
  } = {};
  public [domTypes.eventTargetMode] = "";
  public [domTypes.eventTargetNodeType]: domTypes.NodeType =
    domTypes.NodeType.DOCUMENT_FRAGMENT_NODE;
  private [eventTargetAssignedSlot] = false;
  private [eventTargetHasActivationBehavior] = false;

  public addEventListener(
    type: string,
    callback: domTypes.EventListenerOrEventListenerObject | null,
    options?: domTypes.AddEventListenerOptions | boolean
  ): void {
    const this_ = this || globalThis;

    requiredArguments("EventTarget.addEventListener", arguments.length, 2);
    const normalizedOptions: domTypes.AddEventListenerOptions = eventTargetHelpers.normalizeAddEventHandlerOptions(
      options
    );

    if (callback === null) {
      return;
    }

    const listeners = this_[domTypes.eventTargetListeners];

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

    listeners[type].push({
      callback,
      options: normalizedOptions,
    });
  }

  public removeEventListener(
    type: string,
    callback: domTypes.EventListenerOrEventListenerObject | null,
    options?: domTypes.EventListenerOptions | boolean
  ): void {
    const this_ = this || globalThis;

    requiredArguments("EventTarget.removeEventListener", arguments.length, 2);
    const listeners = this_[domTypes.eventTargetListeners];
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
    const this_ = this || globalThis;

    requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
    const listeners = this_[domTypes.eventTargetListeners];
    if (!hasOwnProperty(listeners, event.type)) {
      return true;
    }

    if (event.dispatched || !event.initialized) {
      // TODO(bartlomieju): very likely that different error
      // should be thrown here (DOMException?)
      throw new TypeError("Tried to dispatch an uninitialized event");
    }

    if (event.eventPhase !== domTypes.EventPhase.NONE) {
      // TODO(bartlomieju): very likely that different error
      // should be thrown here (DOMException?)
      throw new TypeError("Tried to dispatch a dispatching event");
    }

    return eventTargetHelpers.dispatch(this_, event);
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
    targetListeners: { [type in string]: domTypes.EventTargetListener[] }
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
        if (typeof listener.callback === "object") {
          if (typeof listener.callback.handleEvent === "function") {
            listener.callback.handleEvent(eventImpl);
          }
        } else {
          listener.callback.call(eventImpl.currentTarget, eventImpl);
        }
      } catch (error) {
        // TODO(bartlomieju): very likely that different error
        // should be thrown here (DOMException?)
        throw new Error(error.message);
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
        passive: false,
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
        capture: Boolean(options),
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
      slotInClosedTree,
    });
  },
};

Reflect.defineProperty(EventTarget.prototype, "addEventListener", {
  enumerable: true,
});
Reflect.defineProperty(EventTarget.prototype, "removeEventListener", {
  enumerable: true,
});
Reflect.defineProperty(EventTarget.prototype, "dispatchEvent", {
  enumerable: true,
});
