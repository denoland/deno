// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module follows most of the WHATWG Living Standard for the DOM logic.
// Many parts of the DOM are not implemented in Deno, but the logic for those
// parts still exists.  This means you will observe a lot of strange structures
// and impossible logic branches based on what Deno currently supports.

((window) => {
  const eventData = new WeakMap();

  function requiredArguments(
    name,
    length,
    required,
  ) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  // accessors for non runtime visible data

  function getDispatched(event) {
    return Boolean(eventData.get(event)?.dispatched);
  }

  function getPath(event) {
    return eventData.get(event)?.path ?? [];
  }

  function getStopImmediatePropagation(event) {
    return Boolean(eventData.get(event)?.stopImmediatePropagation);
  }

  function setCurrentTarget(
    event,
    value,
  ) {
    event.currentTarget = value;
  }

  function setIsTrusted(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.isTrusted = value;
    }
  }

  function setDispatched(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.dispatched = value;
    }
  }

  function setEventPhase(event, value) {
    event.eventPhase = value;
  }

  function setInPassiveListener(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.inPassiveListener = value;
    }
  }

  function setPath(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.path = value;
    }
  }

  function setRelatedTarget(
    event,
    value,
  ) {
    if ("relatedTarget" in event) {
      event.relatedTarget = value;
    }
  }

  function setTarget(event, value) {
    event.target = value;
  }

  function setStopImmediatePropagation(
    event,
    value,
  ) {
    const data = eventData.get(event);
    if (data) {
      data.stopImmediatePropagation = value;
    }
  }

  // Type guards that widen the event type

  function hasRelatedTarget(
    event,
  ) {
    return "relatedTarget" in event;
  }

  const isTrusted = Object.getOwnPropertyDescriptor({
    get isTrusted() {
      return eventData.get(this).isTrusted;
    },
  }, "isTrusted").get;

  class Event {
    #canceledFlag = false;
    #stopPropagationFlag = false;
    #attributes = {};

    constructor(type, eventInitDict = {}) {
      requiredArguments("Event", arguments.length, 1);
      type = String(type);
      this.#attributes = {
        type,
        bubbles: eventInitDict.bubbles ?? false,
        cancelable: eventInitDict.cancelable ?? false,
        composed: eventInitDict.composed ?? false,
        currentTarget: null,
        eventPhase: Event.NONE,
        target: null,
        timeStamp: Date.now(),
      };
      eventData.set(this, {
        dispatched: false,
        inPassiveListener: false,
        isTrusted: false,
        path: [],
        stopImmediatePropagation: false,
      });
      Reflect.defineProperty(this, "isTrusted", {
        enumerable: true,
        get: isTrusted,
      });
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, EVENT_PROPS);
    }

    get bubbles() {
      return this.#attributes.bubbles;
    }

    get cancelBubble() {
      return this.#stopPropagationFlag;
    }

    set cancelBubble(value) {
      this.#stopPropagationFlag = value;
    }

    get cancelable() {
      return this.#attributes.cancelable;
    }

    get composed() {
      return this.#attributes.composed;
    }

    get currentTarget() {
      return this.#attributes.currentTarget;
    }

    set currentTarget(value) {
      this.#attributes = {
        type: this.type,
        bubbles: this.bubbles,
        cancelable: this.cancelable,
        composed: this.composed,
        currentTarget: value,
        eventPhase: this.eventPhase,
        target: this.target,
        timeStamp: this.timeStamp,
      };
    }

    get defaultPrevented() {
      return this.#canceledFlag;
    }

    get eventPhase() {
      return this.#attributes.eventPhase;
    }

    set eventPhase(value) {
      this.#attributes = {
        type: this.type,
        bubbles: this.bubbles,
        cancelable: this.cancelable,
        composed: this.composed,
        currentTarget: this.currentTarget,
        eventPhase: value,
        target: this.target,
        timeStamp: this.timeStamp,
      };
    }

    get initialized() {
      return true;
    }

    get target() {
      return this.#attributes.target;
    }

    set target(value) {
      this.#attributes = {
        type: this.type,
        bubbles: this.bubbles,
        cancelable: this.cancelable,
        composed: this.composed,
        currentTarget: this.currentTarget,
        eventPhase: this.eventPhase,
        target: value,
        timeStamp: this.timeStamp,
      };
    }

    get timeStamp() {
      return this.#attributes.timeStamp;
    }

    get type() {
      return this.#attributes.type;
    }

    composedPath() {
      const path = eventData.get(this).path;
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

      for (let index = currentTargetIndex + 1; index < path.length; index++) {
        const { item, rootOfClosedTree, slotInClosedTree } = path[index];

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
      return composedPath.map((p) => p.item);
    }

    preventDefault() {
      if (this.cancelable && !eventData.get(this).inPassiveListener) {
        this.#canceledFlag = true;
      }
    }

    stopPropagation() {
      this.#stopPropagationFlag = true;
    }

    stopImmediatePropagation() {
      this.#stopPropagationFlag = true;
      eventData.get(this).stopImmediatePropagation = true;
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
  }

  function buildCustomInspectOutput(obj, props) {
    const inspectObj = {};

    for (const prop of props) {
      inspectObj[prop] = obj[prop];
    }

    return `${obj.constructor.name} ${Deno.inspect(inspectObj)}`;
  }

  function defineEnumerableProps(
    Ctor,
    props,
  ) {
    for (const prop of props) {
      Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
    }
  }

  const EVENT_PROPS = [
    "bubbles",
    "cancelable",
    "composed",
    "currentTarget",
    "defaultPrevented",
    "eventPhase",
    "target",
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
    return Boolean(eventTarget && "nodeType" in eventTarget);
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

  function isSlotable(
    nodeImpl,
  ) {
    return Boolean(isNode(nodeImpl) && "assignedSlot" in nodeImpl);
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
    targetImpl,
    eventImpl,
    targetOverride,
  ) {
    let clearTargets = false;
    let activationTarget = null;

    setDispatched(eventImpl, true);

    targetOverride = targetOverride ?? targetImpl;
    const eventRelatedTarget = hasRelatedTarget(eventImpl)
      ? eventImpl.relatedTarget
      : null;
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
      let slotable = isSlotable(targetImpl) && getAssignedSlot(targetImpl)
        ? targetImpl
        : null;
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
    eventImpl,
    targetListeners,
  ) {
    let found = false;

    const { type } = eventImpl;

    if (!targetListeners || !targetListeners[type]) {
      return found;
    }

    // Copy event listeners before iterating since the list can be modified during the iteration.
    const handlers = targetListeners[type].slice();

    for (let i = 0; i < handlers.length; i++) {
      const listener = handlers[i];

      let capture, once, passive, signal;
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
        targetListeners[type].splice(
          targetListeners[type].indexOf(listener),
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
  function invokeEventListeners(tuple, eventImpl) {
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
    options,
  ) {
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

  const eventTargetData = new WeakMap();

  function setEventTargetData(value) {
    eventTargetData.set(value, getDefaultTargetData());
  }

  function getAssignedSlot(target) {
    return Boolean(eventTargetData.get(target)?.assignedSlot);
  }

  function getHasActivationBehavior(target) {
    return Boolean(
      eventTargetData.get(target)?.hasActivationBehavior,
    );
  }

  function getHost(target) {
    return eventTargetData.get(target)?.host ?? null;
  }

  function getListeners(target) {
    return eventTargetData.get(target)?.listeners ?? {};
  }

  function getMode(target) {
    return eventTargetData.get(target)?.mode ?? null;
  }

  function getDefaultTargetData() {
    return {
      assignedSlot: false,
      hasActivationBehavior: false,
      host: null,
      listeners: Object.create(null),
      mode: "",
    };
  }

  class EventTarget {
    constructor() {
      eventTargetData.set(this, getDefaultTargetData());
    }

    addEventListener(
      type,
      callback,
      options,
    ) {
      requiredArguments("EventTarget.addEventListener", arguments.length, 2);
      if (callback === null) {
        return;
      }

      options = normalizeAddEventHandlerOptions(options);
      const { listeners } = eventTargetData.get(this ?? globalThis);

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
      if (options?.signal) {
        const signal = options?.signal;
        if (signal.aborted) {
          // If signal is not null and its aborted flag is set, then return.
          return;
        } else {
          // If listener’s signal is not null, then add the following abort
          // abort steps to it: Remove an event listener.
          signal.addEventListener("abort", () => {
            this.removeEventListener(type, callback, options);
          });
        }
      }
      listeners[type].push({ callback, options });
    }

    removeEventListener(
      type,
      callback,
      options,
    ) {
      requiredArguments("EventTarget.removeEventListener", arguments.length, 2);

      const listeners = eventTargetData.get(this ?? globalThis).listeners;
      if (callback !== null && type in listeners) {
        listeners[type] = listeners[type].filter(
          (listener) => listener.callback !== callback,
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

    dispatchEvent(event) {
      requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
      const self = this ?? globalThis;

      const listeners = eventTargetData.get(self).listeners;
      if (!(event.type in listeners)) {
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

    get [Symbol.toStringTag]() {
      return "EventTarget";
    }

    getParent(_event) {
      return null;
    }
  }

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
        error = null,
      } = {},
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

    get [Symbol.toStringTag]() {
      return "ErrorEvent";
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, [
        ...EVENT_PROPS,
        "message",
        "filename",
        "lineno",
        "colno",
        "error",
      ]);
    }
  }

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
    } = {}) {
      super(type, {
        bubbles: bubbles,
        cancelable: cancelable,
        composed: composed,
      });

      this.#wasClean = wasClean;
      this.#code = code;
      this.#reason = reason;
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, [
        ...EVENT_PROPS,
        "wasClean",
        "code",
        "reason",
      ]);
    }
  }

  class MessageEvent extends Event {
    constructor(type, eventInitDict) {
      super(type, {
        bubbles: eventInitDict?.bubbles ?? false,
        cancelable: eventInitDict?.cancelable ?? false,
        composed: eventInitDict?.composed ?? false,
      });

      this.data = eventInitDict?.data ?? null;
      this.origin = eventInitDict?.origin ?? "";
      this.lastEventId = eventInitDict?.lastEventId ?? "";
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, [
        ...EVENT_PROPS,
        "data",
        "origin",
        "lastEventId",
      ]);
    }
  }

  class StorageEvent extends Event {
    constructor(type, eventInitDict) {
      super(type, eventInitDict);

      this.key = eventInitDict.key;
      this.oldValue = eventInitDict.oldValue;
      this.newValue = eventInitDict.newValue;
      this.url = eventInitDict.url;
      this.storageArea = eventInitDict.storageArea;
    }
  }

  class CustomEvent extends Event {
    #detail = null;

    constructor(type, eventInitDict = {}) {
      super(type, eventInitDict);
      requiredArguments("CustomEvent", arguments.length, 1);
      const { detail } = eventInitDict;
      this.#detail = detail;
    }

    get detail() {
      return this.#detail;
    }

    get [Symbol.toStringTag]() {
      return "CustomEvent";
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, [
        ...EVENT_PROPS,
        "detail",
      ]);
    }
  }

  Reflect.defineProperty(CustomEvent.prototype, "detail", {
    enumerable: true,
  });

  // ProgressEvent could also be used in other DOM progress event emits.
  // Current use is for FileReader.
  class ProgressEvent extends Event {
    constructor(type, eventInitDict = {}) {
      super(type, eventInitDict);

      this.lengthComputable = eventInitDict?.lengthComputable ?? false;
      this.loaded = eventInitDict?.loaded ?? 0;
      this.total = eventInitDict?.total ?? 0;
    }

    [Symbol.for("Deno.customInspect")]() {
      return buildCustomInspectOutput(this, [
        ...EVENT_PROPS,
        "lengthComputable",
        "loaded",
        "total",
      ]);
    }
  }

  window.Event = Event;
  window.EventTarget = EventTarget;
  window.ErrorEvent = ErrorEvent;
  window.CloseEvent = CloseEvent;
  window.MessageEvent = MessageEvent;
  window.StorageEvent = StorageEvent;
  window.CustomEvent = CustomEvent;
  window.ProgressEvent = ProgressEvent;
  window.dispatchEvent = EventTarget.prototype.dispatchEvent;
  window.addEventListener = EventTarget.prototype.addEventListener;
  window.removeEventListener = EventTarget.prototype.removeEventListener;
  window.__bootstrap = (window.__bootstrap || {});
  window.__bootstrap.eventTarget = {
    setEventTargetData,
  };
  window.__bootstrap.event = {
    setIsTrusted,
  };
})(this);
