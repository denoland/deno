// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/web/event.ts",
  ["$deno$/web/util.ts", "$deno$/util.ts"],
  function (exports_85, context_85) {
    "use strict";
    let util_ts_10, util_ts_11, eventData, EventImpl;
    const __moduleName = context_85 && context_85.id;
    // accessors for non runtime visible data
    function getDispatched(event) {
      return Boolean(eventData.get(event)?.dispatched);
    }
    exports_85("getDispatched", getDispatched);
    function getPath(event) {
      return eventData.get(event)?.path ?? [];
    }
    exports_85("getPath", getPath);
    function getStopImmediatePropagation(event) {
      return Boolean(eventData.get(event)?.stopImmediatePropagation);
    }
    exports_85("getStopImmediatePropagation", getStopImmediatePropagation);
    function setCurrentTarget(event, value) {
      event.currentTarget = value;
    }
    exports_85("setCurrentTarget", setCurrentTarget);
    function setDispatched(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.dispatched = value;
      }
    }
    exports_85("setDispatched", setDispatched);
    function setEventPhase(event, value) {
      event.eventPhase = value;
    }
    exports_85("setEventPhase", setEventPhase);
    function setInPassiveListener(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.inPassiveListener = value;
      }
    }
    exports_85("setInPassiveListener", setInPassiveListener);
    function setPath(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.path = value;
      }
    }
    exports_85("setPath", setPath);
    function setRelatedTarget(event, value) {
      if ("relatedTarget" in event) {
        event.relatedTarget = value;
      }
    }
    exports_85("setRelatedTarget", setRelatedTarget);
    function setTarget(event, value) {
      event.target = value;
    }
    exports_85("setTarget", setTarget);
    function setStopImmediatePropagation(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.stopImmediatePropagation = value;
      }
    }
    exports_85("setStopImmediatePropagation", setStopImmediatePropagation);
    // Type guards that widen the event type
    function hasRelatedTarget(event) {
      return "relatedTarget" in event;
    }
    exports_85("hasRelatedTarget", hasRelatedTarget);
    function isTrusted() {
      return eventData.get(this).isTrusted;
    }
    return {
      setters: [
        function (util_ts_10_1) {
          util_ts_10 = util_ts_10_1;
        },
        function (util_ts_11_1) {
          util_ts_11 = util_ts_11_1;
        },
      ],
      execute: function () {
        eventData = new WeakMap();
        EventImpl = class EventImpl {
          constructor(type, eventInitDict = {}) {
            this.#canceledFlag = false;
            this.#stopPropagationFlag = false;
            util_ts_10.requiredArguments("Event", arguments.length, 1);
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
          #canceledFlag;
          #stopPropagationFlag;
          #attributes;
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
            util_ts_11.assert(this.currentTarget);
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
            for (
              let index = currentTargetIndex + 1;
              index < path.length;
              index++
            ) {
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
        };
        exports_85("EventImpl", EventImpl);
        util_ts_10.defineEnumerableProps(EventImpl, [
          "bubbles",
          "cancelable",
          "composed",
          "currentTarget",
          "defaultPrevented",
          "eventPhase",
          "target",
          "timeStamp",
          "type",
        ]);
      },
    };
  }
);
System.register(
  "$deno$/web/custom_event.ts",
  ["$deno$/web/event.ts", "$deno$/web/util.ts"],
  function (exports_86, context_86) {
    "use strict";
    let event_ts_1, util_ts_12, CustomEventImpl;
    const __moduleName = context_86 && context_86.id;
    return {
      setters: [
        function (event_ts_1_1) {
          event_ts_1 = event_ts_1_1;
        },
        function (util_ts_12_1) {
          util_ts_12 = util_ts_12_1;
        },
      ],
      execute: function () {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        CustomEventImpl = class CustomEventImpl extends event_ts_1.EventImpl {
          constructor(type, eventInitDict = {}) {
            super(type, eventInitDict);
            util_ts_12.requiredArguments("CustomEvent", arguments.length, 1);
            const { detail } = eventInitDict;
            this.#detail = detail;
          }
          #detail;
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          get detail() {
            return this.#detail;
          }
          get [Symbol.toStringTag]() {
            return "CustomEvent";
          }
        };
        exports_86("CustomEventImpl", CustomEventImpl);
        Reflect.defineProperty(CustomEventImpl.prototype, "detail", {
          enumerable: true,
        });
      },
    };
  }
);
System.register(
  "$deno$/web/event_target.ts",
  ["$deno$/web/dom_exception.ts", "$deno$/web/event.ts", "$deno$/web/util.ts"],
  function (exports_89, context_89) {
    "use strict";
    let dom_exception_ts_1,
      event_ts_2,
      util_ts_13,
      DOCUMENT_FRAGMENT_NODE,
      eventTargetData,
      EventTargetImpl;
    const __moduleName = context_89 && context_89.id;
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
    function isNode(eventTarget) {
      return Boolean(eventTarget && "nodeType" in eventTarget);
    }
    // https://dom.spec.whatwg.org/#concept-shadow-including-inclusive-ancestor
    function isShadowInclusiveAncestor(ancestor, node) {
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
          getHost(nodeImpl) != null
      );
    }
    function isSlotable(nodeImpl) {
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
      slotInClosedTree
    ) {
      const itemInShadowTree = isNode(target) && isShadowRoot(getRoot(target));
      const rootOfClosedTree =
        isShadowRoot(target) && getMode(target) === "closed";
      event_ts_2.getPath(eventImpl).push({
        item: target,
        itemInShadowTree,
        target: targetOverride,
        relatedTarget,
        touchTargetList: touchTargets,
        rootOfClosedTree,
        slotInClosedTree,
      });
    }
    function dispatch(targetImpl, eventImpl, targetOverride) {
      let clearTargets = false;
      let activationTarget = null;
      event_ts_2.setDispatched(eventImpl, true);
      targetOverride = targetOverride ?? targetImpl;
      const eventRelatedTarget = event_ts_2.hasRelatedTarget(eventImpl)
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
          false
        );
        const isActivationEvent = eventImpl.type === "click";
        if (isActivationEvent && getHasActivationBehavior(targetImpl)) {
          activationTarget = targetImpl;
        }
        let slotInClosedTree = false;
        let slotable =
          isSlotable(targetImpl) && getAssignedSlot(targetImpl)
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
        const path = event_ts_2.getPath(eventImpl);
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
        event_ts_2.setEventPhase(
          eventImpl,
          event_ts_2.EventImpl.CAPTURING_PHASE
        );
        for (let i = path.length - 1; i >= 0; --i) {
          const tuple = path[i];
          if (tuple.target === null) {
            invokeEventListeners(tuple, eventImpl);
          }
        }
        for (let i = 0; i < path.length; i++) {
          const tuple = path[i];
          if (tuple.target !== null) {
            event_ts_2.setEventPhase(eventImpl, event_ts_2.EventImpl.AT_TARGET);
          } else {
            event_ts_2.setEventPhase(
              eventImpl,
              event_ts_2.EventImpl.BUBBLING_PHASE
            );
          }
          if (
            (eventImpl.eventPhase === event_ts_2.EventImpl.BUBBLING_PHASE &&
              eventImpl.bubbles) ||
            eventImpl.eventPhase === event_ts_2.EventImpl.AT_TARGET
          ) {
            invokeEventListeners(tuple, eventImpl);
          }
        }
      }
      event_ts_2.setEventPhase(eventImpl, event_ts_2.EventImpl.NONE);
      event_ts_2.setCurrentTarget(eventImpl, null);
      event_ts_2.setPath(eventImpl, []);
      event_ts_2.setDispatched(eventImpl, false);
      eventImpl.cancelBubble = false;
      event_ts_2.setStopImmediatePropagation(eventImpl, false);
      if (clearTargets) {
        event_ts_2.setTarget(eventImpl, null);
        event_ts_2.setRelatedTarget(eventImpl, null);
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
    function innerInvokeEventListeners(eventImpl, targetListeners) {
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
          (eventImpl.eventPhase === event_ts_2.EventImpl.CAPTURING_PHASE &&
            !capture) ||
          (eventImpl.eventPhase === event_ts_2.EventImpl.BUBBLING_PHASE &&
            capture)
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
          event_ts_2.setInPassiveListener(eventImpl, true);
        }
        if (typeof listener.callback === "object") {
          if (typeof listener.callback.handleEvent === "function") {
            listener.callback.handleEvent(eventImpl);
          }
        } else {
          listener.callback.call(eventImpl.currentTarget, eventImpl);
        }
        event_ts_2.setInPassiveListener(eventImpl, false);
        if (event_ts_2.getStopImmediatePropagation(eventImpl)) {
          return found;
        }
      }
      return found;
    }
    /** Invokes the listeners on a given event path with the supplied event.
     *
     * Ref: https://dom.spec.whatwg.org/#concept-event-listener-invoke */
    function invokeEventListeners(tuple, eventImpl) {
      const path = event_ts_2.getPath(eventImpl);
      const tupleIndex = path.indexOf(tuple);
      for (let i = tupleIndex; i >= 0; i--) {
        const t = path[i];
        if (t.target) {
          event_ts_2.setTarget(eventImpl, t.target);
          break;
        }
      }
      event_ts_2.setRelatedTarget(eventImpl, tuple.relatedTarget);
      if (eventImpl.cancelBubble) {
        return;
      }
      event_ts_2.setCurrentTarget(eventImpl, tuple.item);
      innerInvokeEventListeners(eventImpl, getListeners(tuple.item));
    }
    function normalizeAddEventHandlerOptions(options) {
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
    function normalizeEventHandlerOptions(options) {
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
    function getAssignedSlot(target) {
      return Boolean(eventTargetData.get(target)?.assignedSlot);
    }
    function getHasActivationBehavior(target) {
      return Boolean(eventTargetData.get(target)?.hasActivationBehavior);
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
    exports_89("getDefaultTargetData", getDefaultTargetData);
    return {
      setters: [
        function (dom_exception_ts_1_1) {
          dom_exception_ts_1 = dom_exception_ts_1_1;
        },
        function (event_ts_2_1) {
          event_ts_2 = event_ts_2_1;
        },
        function (util_ts_13_1) {
          util_ts_13 = util_ts_13_1;
        },
      ],
      execute: function () {
        // This is currently the only node type we are using, so instead of implementing
        // the whole of the Node interface at the moment, this just gives us the one
        // value to power the standards based logic
        DOCUMENT_FRAGMENT_NODE = 11;
        // Accessors for non-public data
        exports_89("eventTargetData", (eventTargetData = new WeakMap()));
        EventTargetImpl = class EventTargetImpl {
          constructor() {
            eventTargetData.set(this, getDefaultTargetData());
          }
          addEventListener(type, callback, options) {
            util_ts_13.requiredArguments(
              "EventTarget.addEventListener",
              arguments.length,
              2
            );
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
            listeners[type].push({ callback, options });
          }
          removeEventListener(type, callback, options) {
            util_ts_13.requiredArguments(
              "EventTarget.removeEventListener",
              arguments.length,
              2
            );
            const listeners = eventTargetData.get(this ?? globalThis).listeners;
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
          dispatchEvent(event) {
            util_ts_13.requiredArguments(
              "EventTarget.dispatchEvent",
              arguments.length,
              1
            );
            const self = this ?? globalThis;
            const listeners = eventTargetData.get(self).listeners;
            if (!(event.type in listeners)) {
              return true;
            }
            if (event_ts_2.getDispatched(event)) {
              throw new dom_exception_ts_1.DOMExceptionImpl(
                "Invalid event state.",
                "InvalidStateError"
              );
            }
            if (event.eventPhase !== event_ts_2.EventImpl.NONE) {
              throw new dom_exception_ts_1.DOMExceptionImpl(
                "Invalid event state.",
                "InvalidStateError"
              );
            }
            return dispatch(self, event);
          }
          get [Symbol.toStringTag]() {
            return "EventTarget";
          }
          getParent(_event) {
            return null;
          }
        };
        exports_89("EventTargetImpl", EventTargetImpl);
        util_ts_13.defineEnumerableProps(EventTargetImpl, [
          "addEventListener",
          "removeEventListener",
          "dispatchEvent",
        ]);
      },
    };
  }
);
