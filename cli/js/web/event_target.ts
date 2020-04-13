// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module follows most of the WHATWG Living Standard for the DOM logic.
// Many parts of the DOM are not implemented in Deno, but the logic for those
// parts still exists.  This means you will observe a lot of strange structures
// and impossible logic branches based on what Deno currently supports.

import { DOMExceptionImpl as DOMException } from "./dom_exception.ts";
import * as domTypes from "./dom_types.d.ts";
import {
  EventImpl as Event,
  EventPath,
  getDispatched,
  getPath,
  getStopImmediatePropagation,
  hasRelatedTarget,
  setCurrentTarget,
  setDispatched,
  setEventPhase,
  setInPassiveListener,
  setPath,
  setRelatedTarget,
  setStopImmediatePropagation,
  setTarget,
} from "./event.ts";
import { defineEnumerableProps, requiredArguments } from "./util.ts";

// This is currently the only node type we are using, so instead of implementing
// the whole of the Node interface at the moment, this just gives us the one
// value to power the standards based logic
const DOCUMENT_FRAGMENT_NODE = 11;

// DOM Logic Helper functions and type guards

/** Get the parent node, for event targets that have a parent.
 *
 * Ref: https://dom.spec.whatwg.org/#get-the-parent */
function getParent(eventTarget: EventTarget): EventTarget | null {
  return isNode(eventTarget) ? eventTarget.parentNode : null;
}

function getRoot(eventTarget: EventTarget): EventTarget | null {
  return isNode(eventTarget)
    ? eventTarget.getRootNode({ composed: true })
    : null;
}

function isNode<T extends EventTarget>(
  eventTarget: T | null
): eventTarget is T & domTypes.Node {
  return Boolean(eventTarget && "nodeType" in eventTarget);
}

// https://dom.spec.whatwg.org/#concept-shadow-including-inclusive-ancestor
function isShadowInclusiveAncestor(
  ancestor: EventTarget | null,
  node: EventTarget | null
): boolean {
  while (isNode(node)) {
    if (node === ancestor) {
      return true;
    }

    if (isShadowRoot(node)) {
      node = node && getHost(node);
    } else {
      node = getParent(node);
    }
  }

  return false;
}

function isShadowRoot(nodeImpl: EventTarget | null): boolean {
  return Boolean(
    nodeImpl &&
      isNode(nodeImpl) &&
      nodeImpl.nodeType === DOCUMENT_FRAGMENT_NODE &&
      getHost(nodeImpl) != null
  );
}

function isSlotable<T extends EventTarget>(
  nodeImpl: T | null
): nodeImpl is T & domTypes.Node & domTypes.Slotable {
  return Boolean(isNode(nodeImpl) && "assignedSlot" in nodeImpl);
}

// DOM Logic functions

/** Append a path item to an event's path.
 *
 * Ref: https://dom.spec.whatwg.org/#concept-event-path-append
 */
function appendToEventPath(
  eventImpl: Event,
  target: EventTarget,
  targetOverride: EventTarget | null,
  relatedTarget: EventTarget | null,
  touchTargets: EventTarget[],
  slotInClosedTree: boolean
): void {
  const itemInShadowTree = isNode(target) && isShadowRoot(getRoot(target));
  const rootOfClosedTree = isShadowRoot(target) && getMode(target) === "closed";

  getPath(eventImpl).push({
    item: target,
    itemInShadowTree,
    target: targetOverride,
    relatedTarget,
    touchTargetList: touchTargets,
    rootOfClosedTree,
    slotInClosedTree,
  });
}

function dispatch(
  targetImpl: EventTarget,
  eventImpl: Event,
  targetOverride?: EventTarget
): boolean {
  let clearTargets = false;
  let activationTarget: EventTarget | null = null;

  setDispatched(eventImpl, true);

  targetOverride = targetOverride ?? targetImpl;
  const eventRelatedTarget = hasRelatedTarget(eventImpl)
    ? eventImpl.relatedTarget
    : null;
  let relatedTarget = retarget(eventRelatedTarget, targetImpl);

  if (targetImpl !== relatedTarget || targetImpl === eventRelatedTarget) {
    const touchTargets: EventTarget[] = [];

    appendToEventPath(
      eventImpl,
      targetImpl,
      targetOverride,
      relatedTarget,
      touchTargets,
      false
    );

    const isActivationEvent = eventImpl.type === "click";

    if (isActivationEvent && getHasActivationBehavior(targetImpl)) {
      activationTarget = targetImpl;
    }

    let slotInClosedTree = false;
    let slotable =
      isSlotable(targetImpl) && getAssignedSlot(targetImpl) ? targetImpl : null;
    let parent = getParent(targetImpl);

    // Populate event path
    // https://dom.spec.whatwg.org/#event-path
    while (parent !== null) {
      if (slotable !== null) {
        slotable = null;

        const parentRoot = getRoot(parent);
        if (
          isShadowRoot(parentRoot) &&
          parentRoot &&
          getMode(parentRoot) === "closed"
        ) {
          slotInClosedTree = true;
        }
      }

      relatedTarget = retarget(eventRelatedTarget, parent);

      if (
        isNode(parent) &&
        isShadowInclusiveAncestor(getRoot(targetImpl), parent)
      ) {
        appendToEventPath(
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
          getHasActivationBehavior(targetImpl)
        ) {
          activationTarget = targetImpl;
        }

        appendToEventPath(
          eventImpl,
          parent,
          targetImpl,
          relatedTarget,
          touchTargets,
          slotInClosedTree
        );
      }

      if (parent !== null) {
        parent = getParent(parent);
      }

      slotInClosedTree = false;
    }

    let clearTargetsTupleIndex = -1;
    const path = getPath(eventImpl);
    for (
      let i = path.length - 1;
      i >= 0 && clearTargetsTupleIndex === -1;
      i--
    ) {
      if (path[i].target !== null) {
        clearTargetsTupleIndex = i;
      }
    }
    const clearTargetsTuple = path[clearTargetsTupleIndex];

    clearTargets =
      (isNode(clearTargetsTuple.target) &&
        isShadowRoot(getRoot(clearTargetsTuple.target))) ||
      (isNode(clearTargetsTuple.relatedTarget) &&
        isShadowRoot(getRoot(clearTargetsTuple.relatedTarget)));

    setEventPhase(eventImpl, Event.CAPTURING_PHASE);

    for (let i = path.length - 1; i >= 0; --i) {
      const tuple = path[i];

      if (tuple.target === null) {
        invokeEventListeners(tuple, eventImpl);
      }
    }

    for (let i = 0; i < path.length; i++) {
      const tuple = path[i];

      if (tuple.target !== null) {
        setEventPhase(eventImpl, Event.AT_TARGET);
      } else {
        setEventPhase(eventImpl, Event.BUBBLING_PHASE);
      }

      if (
        (eventImpl.eventPhase === Event.BUBBLING_PHASE && eventImpl.bubbles) ||
        eventImpl.eventPhase === Event.AT_TARGET
      ) {
        invokeEventListeners(tuple, eventImpl);
      }
    }
  }

  setEventPhase(eventImpl, Event.NONE);
  setCurrentTarget(eventImpl, null);
  setPath(eventImpl, []);
  setDispatched(eventImpl, false);
  eventImpl.cancelBubble = false;
  setStopImmediatePropagation(eventImpl, false);

  if (clearTargets) {
    setTarget(eventImpl, null);
    setRelatedTarget(eventImpl, null);
  }

  // TODO: invoke activation targets if HTML nodes will be implemented
  // if (activationTarget !== null) {
  //   if (!eventImpl.defaultPrevented) {
  //     activationTarget._activationBehavior();
  //   }
  // }

  return !eventImpl.defaultPrevented;
}

/** Inner invoking of the event listeners where the resolved listeners are
 * called.
 *
 * Ref: https://dom.spec.whatwg.org/#concept-event-listener-inner-invoke */
function innerInvokeEventListeners(
  eventImpl: Event,
  targetListeners: Record<string, Listener[]>
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
      (eventImpl.eventPhase === Event.CAPTURING_PHASE && !capture) ||
      (eventImpl.eventPhase === Event.BUBBLING_PHASE && capture)
    ) {
      continue;
    }

    if (once) {
      targetListeners[type].splice(targetListeners[type].indexOf(listener), 1);
    }

    if (passive) {
      setInPassiveListener(eventImpl, true);
    }

    if (typeof listener.callback === "object") {
      if (typeof listener.callback.handleEvent === "function") {
        listener.callback.handleEvent(eventImpl);
      }
    } else {
      listener.callback.call(eventImpl.currentTarget, eventImpl);
    }

    setInPassiveListener(eventImpl, false);

    if (getStopImmediatePropagation(eventImpl)) {
      return found;
    }
  }

  return found;
}

/** Invokes the listeners on a given event path with the supplied event.
 *
 * Ref: https://dom.spec.whatwg.org/#concept-event-listener-invoke */
function invokeEventListeners(tuple: EventPath, eventImpl: Event): void {
  const path = getPath(eventImpl);
  const tupleIndex = path.indexOf(tuple);
  for (let i = tupleIndex; i >= 0; i--) {
    const t = path[i];
    if (t.target) {
      setTarget(eventImpl, t.target);
      break;
    }
  }

  setRelatedTarget(eventImpl, tuple.relatedTarget);

  if (eventImpl.cancelBubble) {
    return;
  }

  setCurrentTarget(eventImpl, tuple.item);

  innerInvokeEventListeners(eventImpl, getListeners(tuple.item));
}

function normalizeAddEventHandlerOptions(
  options: boolean | AddEventListenerOptions | undefined
): AddEventListenerOptions {
  if (typeof options === "boolean" || typeof options === "undefined") {
    return {
      capture: Boolean(options),
      once: false,
      passive: false,
    };
  } else {
    return options;
  }
}

function normalizeEventHandlerOptions(
  options: boolean | EventListenerOptions | undefined
): EventListenerOptions {
  if (typeof options === "boolean" || typeof options === "undefined") {
    return {
      capture: Boolean(options),
    };
  } else {
    return options;
  }
}

/** Retarget the target following the spec logic.
 *
 * Ref: https://dom.spec.whatwg.org/#retarget */
function retarget(a: EventTarget | null, b: EventTarget): EventTarget | null {
  while (true) {
    if (!isNode(a)) {
      return a;
    }

    const aRoot = a.getRootNode();

    if (aRoot) {
      if (
        !isShadowRoot(aRoot) ||
        (isNode(b) && isShadowInclusiveAncestor(aRoot, b))
      ) {
        return a;
      }

      a = getHost(aRoot);
    }
  }
}

// Non-public state information for an event target that needs to held onto.
// Some of the information should be moved to other entities (like Node,
// ShowRoot, UIElement, etc.).
interface EventTargetData {
  assignedSlot: boolean;
  hasActivationBehavior: boolean;
  host: EventTarget | null;
  listeners: Record<string, Listener[]>;
  mode: string;
}

interface Listener {
  callback: EventListenerOrEventListenerObject;
  options: AddEventListenerOptions;
}

// Accessors for non-public data

export const eventTargetData = new WeakMap<EventTarget, EventTargetData>();

function getAssignedSlot(target: EventTarget): boolean {
  return Boolean(eventTargetData.get(target as EventTarget)?.assignedSlot);
}

function getHasActivationBehavior(target: EventTarget): boolean {
  return Boolean(
    eventTargetData.get(target as EventTarget)?.hasActivationBehavior
  );
}

function getHost(target: EventTarget): EventTarget | null {
  return eventTargetData.get(target as EventTarget)?.host ?? null;
}

function getListeners(target: EventTarget): Record<string, Listener[]> {
  return eventTargetData.get(target as EventTarget)?.listeners ?? {};
}

function getMode(target: EventTarget): string | null {
  return eventTargetData.get(target as EventTarget)?.mode ?? null;
}

export function getDefaultTargetData(): Readonly<EventTargetData> {
  return {
    assignedSlot: false,
    hasActivationBehavior: false,
    host: null,
    listeners: Object.create(null),
    mode: "",
  };
}

export class EventTargetImpl implements EventTarget {
  constructor() {
    eventTargetData.set(this, getDefaultTargetData());
  }

  public addEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: AddEventListenerOptions | boolean
  ): void {
    requiredArguments("EventTarget.addEventListener", arguments.length, 2);
    if (callback === null) {
      return;
    }

    options = normalizeAddEventHandlerOptions(options);
    const { listeners } = eventTargetData.get(this ?? globalThis)!;

    if (!(type in listeners)) {
      listeners[type] = [];
    }

    for (const listener of listeners[type]) {
      if (
        ((typeof listener.options === "boolean" &&
          listener.options === options.capture) ||
          (typeof listener.options === "object" &&
            listener.options.capture === options.capture)) &&
        listener.callback === callback
      ) {
        return;
      }
    }

    listeners[type].push({ callback, options });
  }

  public removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean
  ): void {
    requiredArguments("EventTarget.removeEventListener", arguments.length, 2);

    const listeners = eventTargetData.get(this ?? globalThis)!.listeners;
    if (callback !== null && type in listeners) {
      listeners[type] = listeners[type].filter(
        (listener) => listener.callback !== callback
      );
    } else if (callback === null || !listeners[type]) {
      return;
    }

    options = normalizeEventHandlerOptions(options);

    for (let i = 0; i < listeners[type].length; ++i) {
      const listener = listeners[type][i];
      if (
        ((typeof listener.options === "boolean" &&
          listener.options === options.capture) ||
          (typeof listener.options === "object" &&
            listener.options.capture === options.capture)) &&
        listener.callback === callback
      ) {
        listeners[type].splice(i, 1);
        break;
      }
    }
  }

  public dispatchEvent(event: Event): boolean {
    requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
    const self = this ?? globalThis;

    const listeners = eventTargetData.get(self)!.listeners;
    if (!(event.type in listeners)) {
      return true;
    }

    if (getDispatched(event)) {
      throw new DOMException("Invalid event state.", "InvalidStateError");
    }

    if (event.eventPhase !== Event.NONE) {
      throw new DOMException("Invalid event state.", "InvalidStateError");
    }

    return dispatch(self, event);
  }

  get [Symbol.toStringTag](): string {
    return "EventTarget";
  }

  protected getParent(_event: Event): EventTarget | null {
    return null;
  }
}

defineEnumerableProps(EventTargetImpl, [
  "addEventListener",
  "removeEventListener",
  "dispatchEvent",
]);
