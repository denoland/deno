// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This module follows most of the WHATWG Living Standard for the DOM logic.
// Many parts of the DOM are not implemented in Deno, but the logic for those
// parts still exists.  This means you will observe a lot of strange structures
// and impossible logic branches based on what Deno currently supports.

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  ArrayPrototypeUnshift,
  Boolean,
  Error,
  FunctionPrototypeCall,
  MapPrototypeGet,
  MapPrototypeSet,
  ObjectCreate,
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectPrototypeIsPrototypeOf,
  ReflectDefineProperty,
  SafeArrayIterator,
  SafeMap,
  StringPrototypeStartsWith,
  Symbol,
  SymbolFor,
  SymbolToStringTag,
  TypeError,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "./01_dom_exception.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

// This should be set via setGlobalThis this is required so that if even
// user deletes globalThis it is still usable
let globalThis_;

function saveGlobalThisReference(val) {
  globalThis_ = val;
}

// accessors for non runtime visible data

function getDispatched(event) {
  return Boolean(event[_dispatched]);
}

function getPath(event) {
  return event[_path] ?? [];
}

function getStopImmediatePropagation(event) {
  return Boolean(event[_stopImmediatePropagationFlag]);
}

function setCurrentTarget(
  event,
  value,
) {
  event[_attributes].currentTarget = value;
}

function setIsTrusted(event, value) {
  event[_isTrusted] = value;
}

function setDispatched(event, value) {
  event[_dispatched] = value;
}

function setEventPhase(event, value) {
  event[_attributes].eventPhase = value;
}

function setInPassiveListener(event, value) {
  event[_inPassiveListener] = value;
}

function setPath(event, value) {
  event[_path] = value;
}

function setRelatedTarget(
  event,
  value,
) {
  event[_attributes].relatedTarget = value;
}

function setTarget(event, value) {
  event[_attributes].target = value;
}

function setStopImmediatePropagation(
  event,
  value,
) {
  event[_stopImmediatePropagationFlag] = value;
}

const isTrusted = ObjectGetOwnPropertyDescriptor({
  get isTrusted() {
    return this[_isTrusted];
  },
}, "isTrusted").get;

const _attributes = Symbol("[[attributes]]");
const _canceledFlag = Symbol("[[canceledFlag]]");
const _stopPropagationFlag = Symbol("[[stopPropagationFlag]]");
const _stopImmediatePropagationFlag = Symbol(
  "[[stopImmediatePropagationFlag]]",
);
const _inPassiveListener = Symbol("[[inPassiveListener]]");
const _dispatched = Symbol("[[dispatched]]");
const _isTrusted = Symbol("[[isTrusted]]");
const _path = Symbol("[[path]]");

class Event {
  constructor(type, eventInitDict = { __proto__: null }) {
    // TODO(lucacasonato): remove when this interface is spec aligned
    this[SymbolToStringTag] = "Event";
    this[_canceledFlag] = false;
    this[_stopPropagationFlag] = false;
    this[_stopImmediatePropagationFlag] = false;
    this[_inPassiveListener] = false;
    this[_dispatched] = false;
    this[_isTrusted] = false;
    this[_path] = [];

    webidl.requiredArguments(
      arguments.length,
      1,
      "Failed to construct 'Event'",
    );
    type = webidl.converters.DOMString(
      type,
      "Failed to construct 'Event'",
      "Argument 1",
    );

    this[_attributes] = {
      type,
      bubbles: !!eventInitDict.bubbles,
      cancelable: !!eventInitDict.cancelable,
      composed: !!eventInitDict.composed,
      currentTarget: null,
      eventPhase: Event.NONE,
      target: null,
      timeStamp: 0,
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(EventPrototype, this),
        keys: EVENT_PROPS,
      }),
      inspectOptions,
    );
  }

  get type() {
    return this[_attributes].type;
  }

  get target() {
    return this[_attributes].target;
  }

  get srcElement() {
    return null;
  }

  set srcElement(_) {
    // this member is deprecated
  }

  get currentTarget() {
    return this[_attributes].currentTarget;
  }

  composedPath() {
    const path = this[_path];
    if (path.length === 0) {
      return [];
    }

    if (!this.currentTarget) {
      throw new Error("assertion error");
    }
    const composedPath = [
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

    for (let index = path.length - 1; index >= 0; index--) {
      const { item, rootOfClosedTree, slotInClosedTree } = path[index];

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
      const { item, rootOfClosedTree, slotInClosedTree } = path[i];

      if (rootOfClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        ArrayPrototypeUnshift(composedPath, {
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

    for (let index = currentTargetIndex + 1; index < path.length; index++) {
      const { item, rootOfClosedTree, slotInClosedTree } = path[index];

      if (slotInClosedTree) {
        currentHiddenLevel++;
      }

      if (currentHiddenLevel <= maxHiddenLevel) {
        ArrayPrototypePush(composedPath, {
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
    return ArrayPrototypeMap(composedPath, (p) => p.item);
  }

  get NONE() {
    return Event.NONE;
  }

  get CAPTURING_PHASE() {
    return Event.CAPTURING_PHASE;
  }

  get AT_TARGET() {
    return Event.AT_TARGET;
  }

  get BUBBLING_PHASE() {
    return Event.BUBBLING_PHASE;
  }

  static get NONE() {
    return 0;
  }

  static get CAPTURING_PHASE() {
    return 1;
  }

  static get AT_TARGET() {
    return 2;
  }

  static get BUBBLING_PHASE() {
    return 3;
  }

  get eventPhase() {
    return this[_attributes].eventPhase;
  }

  stopPropagation() {
    this[_stopPropagationFlag] = true;
  }

  get cancelBubble() {
    return this[_stopPropagationFlag];
  }

  set cancelBubble(value) {
    this[_stopPropagationFlag] = webidl.converters.boolean(value);
  }

  stopImmediatePropagation() {
    this[_stopPropagationFlag] = true;
    this[_stopImmediatePropagationFlag] = true;
  }

  get bubbles() {
    return this[_attributes].bubbles;
  }

  get cancelable() {
    return this[_attributes].cancelable;
  }

  get returnValue() {
    return !this[_canceledFlag];
  }

  set returnValue(value) {
    if (!webidl.converters.boolean(value)) {
      this[_canceledFlag] = true;
    }
  }

  preventDefault() {
    if (this[_attributes].cancelable && !this[_inPassiveListener]) {
      this[_canceledFlag] = true;
    }
  }

  get defaultPrevented() {
    return this[_canceledFlag];
  }

  get composed() {
    return this[_attributes].composed;
  }

  get initialized() {
    return true;
  }

  get timeStamp() {
    return this[_attributes].timeStamp;
  }
}

const EventPrototype = Event.prototype;

// Not spec compliant. The spec defines it as [LegacyUnforgeable]
// but doing so has a big performance hit
ReflectDefineProperty(Event.prototype, "isTrusted", {
  __proto__: null,
  enumerable: true,
  get: isTrusted,
});

function defineEnumerableProps(
  Ctor,
  props,
) {
  for (let i = 0; i < props.length; ++i) {
    const prop = props[i];
    ReflectDefineProperty(Ctor.prototype, prop, {
      __proto__: null,
      enumerable: true,
    });
  }
}

const EVENT_PROPS = [
  "bubbles",
  "cancelable",
  "composed",
  "currentTarget",
  "defaultPrevented",
  "eventPhase",
  "srcElement",
  "target",
  "returnValue",
  "timeStamp",
  "type",
];

defineEnumerableProps(Event, EVENT_PROPS);

// This is currently the only node type we are using, so instead of implementing
// the whole of the Node interface at the moment, this just gives us the one
// value to power the standards based logic
const DOCUMENT_FRAGMENT_NODE = 11;

// DOM Logic Helper functions and type guards

/** Get the parent node, for event targets that have a parent.
 *
 * Ref: https://dom.spec.whatwg.org/#get-the-parent */
function getParent(eventTarget) {
  return isNode(eventTarget) ? eventTarget.parentNode : null;
}

function getRoot(eventTarget) {
  return isNode(eventTarget)
    ? eventTarget.getRootNode({ composed: true })
    : null;
}

function isNode(
  eventTarget,
) {
  return eventTarget?.nodeType !== undefined;
}

// https://dom.spec.whatwg.org/#concept-shadow-including-inclusive-ancestor
function isShadowInclusiveAncestor(
  ancestor,
  node,
) {
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

function isShadowRoot(nodeImpl) {
  return Boolean(
    nodeImpl &&
      isNode(nodeImpl) &&
      nodeImpl.nodeType === DOCUMENT_FRAGMENT_NODE &&
      getHost(nodeImpl) != null,
  );
}

function isSlottable(
  /* nodeImpl, */
) {
  // TODO(marcosc90) currently there aren't any slottables nodes
  // https://dom.spec.whatwg.org/#concept-slotable
  // return isNode(nodeImpl) && ReflectHas(nodeImpl, "assignedSlot");
  return false;
}

// DOM Logic functions

/** Append a path item to an event's path.
 *
 * Ref: https://dom.spec.whatwg.org/#concept-event-path-append
 */
function appendToEventPath(
  eventImpl,
  target,
  targetOverride,
  relatedTarget,
  touchTargets,
  slotInClosedTree,
) {
  const itemInShadowTree = isNode(target) && isShadowRoot(getRoot(target));
  const rootOfClosedTree = isShadowRoot(target) &&
    getMode(target) === "closed";

  ArrayPrototypePush(getPath(eventImpl), {
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
  targetImpl,
  eventImpl,
  targetOverride,
) {
  let clearTargets = false;
  let activationTarget = null;

  setDispatched(eventImpl, true);

  targetOverride = targetOverride ?? targetImpl;
  const eventRelatedTarget = eventImpl.relatedTarget;
  let relatedTarget = retarget(eventRelatedTarget, targetImpl);

  if (targetImpl !== relatedTarget || targetImpl === eventRelatedTarget) {
    const touchTargets = [];

    appendToEventPath(
      eventImpl,
      targetImpl,
      targetOverride,
      relatedTarget,
      touchTargets,
      false,
    );

    const isActivationEvent = eventImpl.type === "click";

    if (isActivationEvent && getHasActivationBehavior(targetImpl)) {
      activationTarget = targetImpl;
    }

    let slotInClosedTree = false;
    let slottable = isSlottable(targetImpl) && getAssignedSlot(targetImpl)
      ? targetImpl
      : null;
    let parent = getParent(targetImpl);

    // Populate event path
    // https://dom.spec.whatwg.org/#event-path
    while (parent !== null) {
      if (slottable !== null) {
        slottable = null;

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
          slotInClosedTree,
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
          slotInClosedTree,
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

    clearTargets = (isNode(clearTargetsTuple.target) &&
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
        (eventImpl.eventPhase === Event.BUBBLING_PHASE &&
          eventImpl.bubbles) ||
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

  // TODO(bartlomieju): invoke activation targets if HTML nodes will be implemented
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
  eventImpl,
  targetListeners,
) {
  let found = false;

  const { type } = eventImpl;

  if (!targetListeners || !targetListeners[type]) {
    return found;
  }

  let handlers = targetListeners[type];
  const handlersLength = handlers.length;

  // Copy event listeners before iterating since the list can be modified during the iteration.
  if (handlersLength > 1) {
    handlers = ArrayPrototypeSlice(targetListeners[type]);
  }

  for (let i = 0; i < handlersLength; i++) {
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
    if (!ArrayPrototypeIncludes(targetListeners[type], listener)) {
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
      ArrayPrototypeSplice(
        targetListeners[type],
        ArrayPrototypeIndexOf(targetListeners[type], listener),
        1,
      );
    }

    if (passive) {
      setInPassiveListener(eventImpl, true);
    }

    if (typeof listener.callback === "object") {
      if (typeof listener.callback.handleEvent === "function") {
        listener.callback.handleEvent(eventImpl);
      }
    } else {
      FunctionPrototypeCall(
        listener.callback,
        eventImpl.currentTarget,
        eventImpl,
      );
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
function invokeEventListeners(tuple, eventImpl) {
  const path = getPath(eventImpl);
  if (path.length === 1) {
    const t = path[0];
    if (t.target) {
      setTarget(eventImpl, t.target);
    }
  } else {
    const tupleIndex = ArrayPrototypeIndexOf(path, tuple);
    for (let i = tupleIndex; i >= 0; i--) {
      const t = path[i];
      if (t.target) {
        setTarget(eventImpl, t.target);
        break;
      }
    }
  }

  setRelatedTarget(eventImpl, tuple.relatedTarget);

  if (eventImpl.cancelBubble) {
    return;
  }

  setCurrentTarget(eventImpl, tuple.item);

  try {
    innerInvokeEventListeners(eventImpl, getListeners(tuple.item));
  } catch (error) {
    reportException(error);
  }
}

function normalizeEventHandlerOptions(
  options,
) {
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
function retarget(a, b) {
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

// Accessors for non-public data

const eventTargetData = Symbol();

function setEventTargetData(target) {
  target[eventTargetData] = getDefaultTargetData();
}

function getAssignedSlot(target) {
  return Boolean(target?.[eventTargetData]?.assignedSlot);
}

function getHasActivationBehavior(target) {
  return Boolean(target?.[eventTargetData]?.hasActivationBehavior);
}

function getHost(target) {
  return target?.[eventTargetData]?.host ?? null;
}

function getListeners(target) {
  return target?.[eventTargetData]?.listeners ?? {};
}

function getMode(target) {
  return target?.[eventTargetData]?.mode ?? null;
}

function listenerCount(target, type) {
  return getListeners(target)?.[type]?.length ?? 0;
}

function getDefaultTargetData() {
  return {
    assignedSlot: false,
    hasActivationBehavior: false,
    host: null,
    listeners: ObjectCreate(null),
    mode: "",
  };
}

function addEventListenerOptionsConverter(V, prefix) {
  if (webidl.type(V) !== "Object") {
    return { capture: !!V, once: false, passive: false };
  }

  const options = {
    capture: !!V.capture,
    once: !!V.once,
    passive: !!V.passive,
  };

  const signal = V.signal;
  if (signal !== undefined) {
    options.signal = webidl.converters.AbortSignal(
      signal,
      prefix,
      "'signal' of 'AddEventListenerOptions' (Argument 3)",
    );
  }

  return options;
}

class EventTarget {
  constructor() {
    this[eventTargetData] = getDefaultTargetData();
    this[webidl.brand] = webidl.brand;
  }

  addEventListener(
    type,
    callback,
    options,
  ) {
    const self = this ?? globalThis_;
    webidl.assertBranded(self, EventTargetPrototype);
    const prefix = "Failed to execute 'addEventListener' on 'EventTarget'";

    webidl.requiredArguments(arguments.length, 2, prefix);

    options = addEventListenerOptionsConverter(options, prefix);

    if (callback === null) {
      return;
    }

    const { listeners } = self[eventTargetData];

    if (!listeners[type]) {
      listeners[type] = [];
    }

    const listenerList = listeners[type];
    for (let i = 0; i < listenerList.length; ++i) {
      const listener = listenerList[i];
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
    if (options?.signal) {
      const signal = options?.signal;
      if (signal.aborted) {
        // If signal is not null and its aborted flag is set, then return.
        return;
      } else {
        // If listener's signal is not null, then add the following abort
        // abort steps to it: Remove an event listener.
        signal.addEventListener("abort", () => {
          self.removeEventListener(type, callback, options);
        });
      }
    }

    ArrayPrototypePush(listeners[type], { callback, options });
  }

  removeEventListener(
    type,
    callback,
    options,
  ) {
    const self = this ?? globalThis_;
    webidl.assertBranded(self, EventTargetPrototype);
    webidl.requiredArguments(
      arguments.length,
      2,
      "Failed to execute 'removeEventListener' on 'EventTarget'",
    );

    const { listeners } = self[eventTargetData];
    if (callback === null || !listeners[type]) {
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
        ArrayPrototypeSplice(listeners[type], i, 1);
        break;
      }
    }
  }

  dispatchEvent(event) {
    // If `this` is not present, then fallback to global scope. We don't use
    // `globalThis` directly here, because it could be deleted by user.
    // Instead use saved reference to global scope when the script was
    // executed.
    const self = this ?? globalThis_;
    webidl.assertBranded(self, EventTargetPrototype);
    webidl.requiredArguments(
      arguments.length,
      1,
      "Failed to execute 'dispatchEvent' on 'EventTarget'",
    );

    // This is an optimization to avoid creating an event listener
    // on each startup.
    // Stores the flag for checking whether unload is dispatched or not.
    // This prevents the recursive dispatches of unload events.
    // See https://github.com/denoland/deno/issues/9201.
    if (event.type === "unload" && self === globalThis_) {
      globalThis_[SymbolFor("Deno.isUnloadDispatched")] = true;
    }

    const { listeners } = self[eventTargetData];
    if (!listeners[event.type]) {
      setTarget(event, this);
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

  getParent(_event) {
    return null;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}

webidl.configureInterface(EventTarget);
const EventTargetPrototype = EventTarget.prototype;

defineEnumerableProps(EventTarget, [
  "addEventListener",
  "removeEventListener",
  "dispatchEvent",
]);

class ErrorEvent extends Event {
  #message = "";
  #filename = "";
  #lineno = "";
  #colno = "";
  #error = "";

  get message() {
    return this.#message;
  }
  get filename() {
    return this.#filename;
  }
  get lineno() {
    return this.#lineno;
  }
  get colno() {
    return this.#colno;
  }
  get error() {
    return this.#error;
  }

  constructor(
    type,
    {
      bubbles,
      cancelable,
      composed,
      message = "",
      filename = "",
      lineno = 0,
      colno = 0,
      error,
    } = { __proto__: null },
  ) {
    super(type, {
      bubbles: bubbles,
      cancelable: cancelable,
      composed: composed,
    });

    this.#message = message;
    this.#filename = filename;
    this.#lineno = lineno;
    this.#colno = colno;
    this.#error = error;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(ErrorEventPrototype, this),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "message",
          "filename",
          "lineno",
          "colno",
          "error",
        ],
      }),
      inspectOptions,
    );
  }

  // TODO(lucacasonato): remove when this interface is spec aligned
  [SymbolToStringTag] = "ErrorEvent";
}

const ErrorEventPrototype = ErrorEvent.prototype;

defineEnumerableProps(ErrorEvent, [
  "message",
  "filename",
  "lineno",
  "colno",
  "error",
]);

class CloseEvent extends Event {
  #wasClean = "";
  #code = "";
  #reason = "";

  get wasClean() {
    return this.#wasClean;
  }
  get code() {
    return this.#code;
  }
  get reason() {
    return this.#reason;
  }

  constructor(type, {
    bubbles,
    cancelable,
    composed,
    wasClean = false,
    code = 0,
    reason = "",
  } = { __proto__: null }) {
    super(type, {
      bubbles: bubbles,
      cancelable: cancelable,
      composed: composed,
    });

    this.#wasClean = wasClean;
    this.#code = code;
    this.#reason = reason;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CloseEventPrototype, this),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "wasClean",
          "code",
          "reason",
        ],
      }),
      inspectOptions,
    );
  }
}

const CloseEventPrototype = CloseEvent.prototype;

class MessageEvent extends Event {
  get source() {
    return null;
  }

  constructor(type, eventInitDict) {
    super(type, {
      bubbles: eventInitDict?.bubbles ?? false,
      cancelable: eventInitDict?.cancelable ?? false,
      composed: eventInitDict?.composed ?? false,
    });

    this.data = eventInitDict?.data ?? null;
    this.ports = eventInitDict?.ports ?? [];
    this.origin = eventInitDict?.origin ?? "";
    this.lastEventId = eventInitDict?.lastEventId ?? "";
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(MessageEventPrototype, this),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "data",
          "origin",
          "lastEventId",
        ],
      }),
      inspectOptions,
    );
  }

  // TODO(lucacasonato): remove when this interface is spec aligned
  [SymbolToStringTag] = "MessageEvent";
}

const MessageEventPrototype = MessageEvent.prototype;

class CustomEvent extends Event {
  #detail = null;

  constructor(type, eventInitDict = { __proto__: null }) {
    super(type, eventInitDict);
    webidl.requiredArguments(
      arguments.length,
      1,
      "Failed to construct 'CustomEvent'",
    );
    const { detail } = eventInitDict;
    this.#detail = detail;
  }

  get detail() {
    return this.#detail;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CustomEventPrototype, this),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "detail",
        ],
      }),
      inspectOptions,
    );
  }

  // TODO(lucacasonato): remove when this interface is spec aligned
  [SymbolToStringTag] = "CustomEvent";
}

const CustomEventPrototype = CustomEvent.prototype;

ReflectDefineProperty(CustomEvent.prototype, "detail", {
  __proto__: null,
  enumerable: true,
});

// ProgressEvent could also be used in other DOM progress event emits.
// Current use is for FileReader.
class ProgressEvent extends Event {
  constructor(type, eventInitDict = { __proto__: null }) {
    super(type, eventInitDict);

    this.lengthComputable = eventInitDict?.lengthComputable ?? false;
    this.loaded = eventInitDict?.loaded ?? 0;
    this.total = eventInitDict?.total ?? 0;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(ProgressEventPrototype, this),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "lengthComputable",
          "loaded",
          "total",
        ],
      }),
      inspectOptions,
    );
  }

  // TODO(lucacasonato): remove when this interface is spec aligned
  [SymbolToStringTag] = "ProgressEvent";
}

const ProgressEventPrototype = ProgressEvent.prototype;

class PromiseRejectionEvent extends Event {
  #promise = null;
  #reason = null;

  get promise() {
    return this.#promise;
  }
  get reason() {
    return this.#reason;
  }

  constructor(
    type,
    {
      bubbles,
      cancelable,
      composed,
      promise,
      reason,
    } = { __proto__: null },
  ) {
    super(type, {
      bubbles: bubbles,
      cancelable: cancelable,
      composed: composed,
    });

    this.#promise = promise;
    this.#reason = reason;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          PromiseRejectionEventPrototype,
          this,
        ),
        keys: [
          ...new SafeArrayIterator(EVENT_PROPS),
          "promise",
          "reason",
        ],
      }),
      inspectOptions,
    );
  }

  // TODO(lucacasonato): remove when this interface is spec aligned
  [SymbolToStringTag] = "PromiseRejectionEvent";
}

const PromiseRejectionEventPrototype = PromiseRejectionEvent.prototype;

defineEnumerableProps(PromiseRejectionEvent, [
  "promise",
  "reason",
]);

const _eventHandlers = Symbol("eventHandlers");

function makeWrappedHandler(handler, isSpecialErrorEventHandler) {
  function wrappedHandler(evt) {
    if (typeof wrappedHandler.handler !== "function") {
      return;
    }

    if (
      isSpecialErrorEventHandler &&
      ObjectPrototypeIsPrototypeOf(ErrorEventPrototype, evt) &&
      evt.type === "error"
    ) {
      const ret = FunctionPrototypeCall(
        wrappedHandler.handler,
        this,
        evt.message,
        evt.filename,
        evt.lineno,
        evt.colno,
        evt.error,
      );
      if (ret === true) {
        evt.preventDefault();
      }
      return;
    }

    return FunctionPrototypeCall(wrappedHandler.handler, this, evt);
  }
  wrappedHandler.handler = handler;
  return wrappedHandler;
}

// `init` is an optional function that will be called the first time that the
// event handler property is set. It will be called with the object on which
// the property is set as its argument.
// `isSpecialErrorEventHandler` can be set to true to opt into the special
// behavior of event handlers for the "error" event in a global scope.
function defineEventHandler(
  emitter,
  name,
  init = undefined,
  isSpecialErrorEventHandler = false,
) {
  // HTML specification section 8.1.7.1
  ObjectDefineProperty(emitter, `on${name}`, {
    __proto__: null,
    get() {
      if (!this[_eventHandlers]) {
        return null;
      }

      return MapPrototypeGet(this[_eventHandlers], name)?.handler ?? null;
    },
    set(value) {
      // All three Web IDL event handler types are nullable callback functions
      // with the [LegacyTreatNonObjectAsNull] extended attribute, meaning
      // anything other than an object is treated as null.
      if (typeof value !== "object" && typeof value !== "function") {
        value = null;
      }

      if (!this[_eventHandlers]) {
        this[_eventHandlers] = new SafeMap();
      }
      let handlerWrapper = MapPrototypeGet(this[_eventHandlers], name);
      if (handlerWrapper) {
        handlerWrapper.handler = value;
      } else if (value !== null) {
        handlerWrapper = makeWrappedHandler(
          value,
          isSpecialErrorEventHandler,
        );
        this.addEventListener(name, handlerWrapper);
        init?.(this);
      }
      MapPrototypeSet(this[_eventHandlers], name, handlerWrapper);
    },
    configurable: true,
    enumerable: true,
  });
}

let reportExceptionStackedCalls = 0;

// https://html.spec.whatwg.org/#report-the-exception
function reportException(error) {
  reportExceptionStackedCalls++;
  const jsError = core.destructureError(error);
  const message = jsError.exceptionMessage;
  let filename = "";
  let lineno = 0;
  let colno = 0;
  if (jsError.frames.length > 0) {
    filename = jsError.frames[0].fileName;
    lineno = jsError.frames[0].lineNumber;
    colno = jsError.frames[0].columnNumber;
  } else {
    const jsError = core.destructureError(new Error());
    const frames = jsError.frames;
    for (let i = 0; i < frames.length; ++i) {
      const frame = frames[i];
      if (
        typeof frame.fileName == "string" &&
        !StringPrototypeStartsWith(frame.fileName, "ext:")
      ) {
        filename = frame.fileName;
        lineno = frame.lineNumber;
        colno = frame.columnNumber;
        break;
      }
    }
  }
  const event = new ErrorEvent("error", {
    cancelable: true,
    message,
    filename,
    lineno,
    colno,
    error,
  });
  // Avoid recursing `reportException()` via error handlers more than once.
  if (reportExceptionStackedCalls > 1 || globalThis_.dispatchEvent(event)) {
    core.reportUnhandledException(error);
  }
  reportExceptionStackedCalls--;
}

function checkThis(thisArg) {
  if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis_) {
    throw new TypeError("Illegal invocation");
  }
}

// https://html.spec.whatwg.org/#dom-reporterror
function reportError(error) {
  checkThis(this);
  const prefix = "Failed to execute 'reportError'";
  webidl.requiredArguments(arguments.length, 1, prefix);
  reportException(error);
}

export {
  CloseEvent,
  CustomEvent,
  defineEventHandler,
  dispatch,
  ErrorEvent,
  Event,
  EventTarget,
  listenerCount,
  MessageEvent,
  ProgressEvent,
  PromiseRejectionEvent,
  reportError,
  reportException,
  saveGlobalThisReference,
  setEventTargetData,
  setIsTrusted,
  setTarget,
};
