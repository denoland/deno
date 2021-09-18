// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// This module follows most of the WHATWG Living Standard for the DOM logic.
// Many parts of the DOM are not implemented in Deno, but the logic for those
// parts still exists.  This means you will observe a lot of strange structures
// and impossible logic branches based on what Deno currently supports.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { DOMException } = window.__bootstrap.domException;
  const consoleInternal = window.__bootstrap.console;
  const {
    ArrayPrototypeFilter,
    ArrayPrototypeIncludes,
    ArrayPrototypeIndexOf,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ArrayPrototypeSlice,
    ArrayPrototypeSplice,
    ArrayPrototypeUnshift,
    Boolean,
    DateNow,
    Error,
    FunctionPrototypeCall,
    Map,
    MapPrototypeGet,
    MapPrototypeSet,
    ObjectCreate,
    ObjectDefineProperty,
    ObjectGetOwnPropertyDescriptor,
    ReflectDefineProperty,
    Symbol,
    SymbolFor,
    SymbolToStringTag,
    TypeError,
    WeakMap,
    WeakMapPrototypeGet,
    WeakMapPrototypeSet,
  } = window.__bootstrap.primordials;

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

  // Type guards that widen the event type

  function hasRelatedTarget(
    event,
  ) {
    return "relatedTarget" in event;
  }

  const isTrusted = ObjectGetOwnPropertyDescriptor({
    get isTrusted() {
      return this[_isTrusted];
    },
  }, "isTrusted").get;

  const eventInitConverter = webidl.createDictionaryConverter("EventInit", [{
    key: "bubbles",
    defaultValue: false,
    converter: webidl.converters.boolean,
  }, {
    key: "cancelable",
    defaultValue: false,
    converter: webidl.converters.boolean,
  }, {
    key: "composed",
    defaultValue: false,
    converter: webidl.converters.boolean,
  }]);

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
    [_attributes] = {};
    [_canceledFlag] = false;
    [_stopPropagationFlag] = false;
    [_stopImmediatePropagationFlag] = false;
    [_inPassiveListener] = false;
    [_dispatched] = false;
    [_isTrusted] = false;
    [_path] = [];

    constructor(type, eventInitDict = {}) {
      webidl.requiredArguments(arguments.length, 1, {
        prefix: "Failed to construct 'Event'",
      });
      type = webidl.converters.DOMString(type, {
        prefix: "Failed to construct 'Event'",
        context: "Argument 1",
      });
      const eventInit = eventInitConverter(eventInitDict, {
        prefix: "Failed to construct 'Event'",
        context: "Argument 2",
      });
      this[_attributes] = {
        type,
        ...eventInit,
        currentTarget: null,
        eventPhase: Event.NONE,
        target: null,
        timeStamp: DateNow(),
      };
      ReflectDefineProperty(this, "isTrusted", {
        enumerable: true,
        get: isTrusted,
      });
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof Event,
        keys: EVENT_PROPS,
      }));
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

  function defineEnumerableProps(
    Ctor,
    props,
  ) {
    for (const prop of props) {
      ReflectDefineProperty(Ctor.prototype, prop, { enumerable: true });
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

    // Copy event listeners before iterating since the list can be modified during the iteration.
    const handlers = ArrayPrototypeSlice(targetListeners[type]);

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
    const tupleIndex = ArrayPrototypeIndexOf(path, tuple);
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
    WeakMapPrototypeSet(eventTargetData, value, getDefaultTargetData());
  }

  function getAssignedSlot(target) {
    return Boolean(WeakMapPrototypeGet(eventTargetData, target)?.assignedSlot);
  }

  function getHasActivationBehavior(target) {
    return Boolean(
      WeakMapPrototypeGet(eventTargetData, target)?.hasActivationBehavior,
    );
  }

  function getHost(target) {
    return WeakMapPrototypeGet(eventTargetData, target)?.host ?? null;
  }

  function getListeners(target) {
    return WeakMapPrototypeGet(eventTargetData, target)?.listeners ?? {};
  }

  function getMode(target) {
    return WeakMapPrototypeGet(eventTargetData, target)?.mode ?? null;
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

  class EventTarget {
    constructor() {
      WeakMapPrototypeSet(eventTargetData, this, getDefaultTargetData());
    }

    addEventListener(
      type,
      callback,
      options,
    ) {
      webidl.requiredArguments(arguments.length, 2, {
        prefix: "Failed to execute 'addEventListener' on 'EventTarget'",
      });
      if (callback === null) {
        return;
      }

      options = normalizeAddEventHandlerOptions(options);
      const { listeners } = WeakMapPrototypeGet(
        eventTargetData,
        this ?? globalThis,
      );

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
      } else if (options?.signal === null) {
        throw new TypeError("signal must be non-null");
      }

      ArrayPrototypePush(listeners[type], { callback, options });
    }

    removeEventListener(
      type,
      callback,
      options,
    ) {
      webidl.requiredArguments(arguments.length, 2, {
        prefix: "Failed to execute 'removeEventListener' on 'EventTarget'",
      });

      const listeners =
        WeakMapPrototypeGet(eventTargetData, this ?? globalThis).listeners;
      if (callback !== null && type in listeners) {
        listeners[type] = ArrayPrototypeFilter(
          listeners[type],
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
          ArrayPrototypeSplice(listeners[type], i, 1);
          break;
        }
      }
    }

    dispatchEvent(event) {
      webidl.requiredArguments(arguments.length, 1, {
        prefix: "Failed to execute 'dispatchEvent' on 'EventTarget'",
      });
      const self = this ?? globalThis;

      const listeners = WeakMapPrototypeGet(eventTargetData, self).listeners;
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

    get [SymbolToStringTag]() {
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

    get [SymbolToStringTag]() {
      return "ErrorEvent";
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof ErrorEvent,
        keys: [
          ...EVENT_PROPS,
          "message",
          "filename",
          "lineno",
          "colno",
          "error",
        ],
      }));
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

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof CloseEvent,
        keys: [
          ...EVENT_PROPS,
          "wasClean",
          "code",
          "reason",
        ],
      }));
    }
  }

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

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof MessageEvent,
        keys: [
          ...EVENT_PROPS,
          "data",
          "origin",
          "lastEventId",
        ],
      }));
    }
  }

  class CustomEvent extends Event {
    #detail = null;

    constructor(type, eventInitDict = {}) {
      super(type, eventInitDict);
      webidl.requiredArguments(arguments.length, 1, {
        prefix: "Failed to construct 'CustomEvent'",
      });
      const { detail } = eventInitDict;
      this.#detail = detail;
    }

    get detail() {
      return this.#detail;
    }

    get [SymbolToStringTag]() {
      return "CustomEvent";
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof CustomEvent,
        keys: [
          ...EVENT_PROPS,
          "detail",
        ],
      }));
    }
  }

  ReflectDefineProperty(CustomEvent.prototype, "detail", {
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

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof ProgressEvent,
        keys: [
          ...EVENT_PROPS,
          "lengthComputable",
          "loaded",
          "total",
        ],
      }));
    }
  }

  const _eventHandlers = Symbol("eventHandlers");

  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return FunctionPrototypeCall(wrappedHandler.handler, this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }

  // TODO(benjamingr) reuse this here and websocket where possible
  function defineEventHandler(emitter, name, init) {
    // HTML specification section 8.1.5.1
    ObjectDefineProperty(emitter, `on${name}`, {
      get() {
        const map = this[_eventHandlers];

        if (!map) return undefined;
        return MapPrototypeGet(map, name)?.handler;
      },
      set(value) {
        if (!this[_eventHandlers]) {
          this[_eventHandlers] = new Map();
        }
        let handlerWrapper = MapPrototypeGet(this[_eventHandlers], name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
          init?.(this);
        }
        MapPrototypeSet(this[_eventHandlers], name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }

  window.Event = Event;
  window.EventTarget = EventTarget;
  window.ErrorEvent = ErrorEvent;
  window.CloseEvent = CloseEvent;
  window.MessageEvent = MessageEvent;
  window.CustomEvent = CustomEvent;
  window.ProgressEvent = ProgressEvent;
  window.dispatchEvent = EventTarget.prototype.dispatchEvent;
  window.addEventListener = EventTarget.prototype.addEventListener;
  window.removeEventListener = EventTarget.prototype.removeEventListener;
  window.__bootstrap.eventTarget = {
    EventTarget,
    setEventTargetData,
  };
  window.__bootstrap.event = {
    setIsTrusted,
    setTarget,
    defineEventHandler,
  };
})(this);
