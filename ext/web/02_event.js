// Copyright 2018-2025 the Deno authors. MIT license.

// This module follows most of the WHATWG Living Standard for the DOM logic.
// Many parts of the DOM are not implemented in Deno, but the logic for those
// parts still exists.  This means you will observe a lot of strange structures
// and impossible logic branches based on what Deno currently supports.

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeFlat,
  Error,
  FunctionPrototypeCall,
  MapPrototypeGet,
  MapPrototypeSet,
  ObjectDefineProperty,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  SafeMap,
  StringPrototypeStartsWith,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;
import {
  CloseEvent,
  CustomEvent,
  ErrorEvent,
  Event,
  EventTarget,
  MessageEvent,
  op_event_dispatch,
  op_event_get_target_listener_count,
  op_event_get_target_listeners,
  op_event_set_is_trusted,
  op_event_set_target,
  op_event_wrap_event_target,
} from "ext:core/ops";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

// This should be set via setGlobalThis this is required so that if even
// user deletes globalThis it is still usable
let globalThis_;

function saveGlobalThisReference(val) {
  globalThis_ = val;
}

function defineEnumerableProps(prototype, props) {
  for (let i = 0; i < props.length; ++i) {
    const prop = props[i];
    ObjectDefineProperty(prototype, prop, {
      __proto__: null,
      enumerable: true,
    });
  }
}

// accessors for non runtime visible data

/**
 * @param {Event} event
 * @param {boolean} value
 */
function setIsTrusted(event, value) {
  op_event_set_is_trusted(event, value);
}

/**
 * @param {Event} event
 * @param {object} value
 */
function setTarget(event, value) {
  op_event_set_target(event, value);
}

ObjectDefineProperties(Event, {
  NONE: {
    __proto__: null,
    value: 0,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  CAPTURING_PHASE: {
    __proto__: null,
    value: 1,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  AT_TARGET: {
    __proto__: null,
    value: 2,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  BUBBLING_PHASE: {
    __proto__: null,
    value: 3,
    writable: false,
    enumerable: true,
    configurable: false,
  },
});

webidl.configureInterface(Event);
const EventPrototype = Event.prototype;

ObjectDefineProperties(Event.prototype, {
  NONE: {
    __proto__: null,
    value: 0,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  CAPTURING_PHASE: {
    __proto__: null,
    value: 1,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  AT_TARGET: {
    __proto__: null,
    value: 2,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  BUBBLING_PHASE: {
    __proto__: null,
    value: 3,
    writable: false,
    enumerable: true,
    configurable: false,
  },
  [SymbolFor("Deno.privateCustomInspect")]: {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(EventPrototype, this),
          keys: EVENT_PROPS,
        }),
        inspectOptions,
      );
    },
  },
});

const EVENT_PROPS = [
  "type",
  "target",
  "currentTarget",
  "eventPhase",
  "bubbles",
  "cancelable",
  "defaultPrevented",
  "composed",
  "timeStamp",
  "srcElement",
  "returnValue",
  "cancelBubble",
  // Not spec compliant. The spec defines it as [LegacyUnforgeable]
  // but doing so has a big performance hit
  "isTrusted",
];

defineEnumerableProps(Event.prototype, EVENT_PROPS);

// Accessors for non-public data

/**
 * @param {object} target
 */
function setEventTargetData(target) {
  op_event_wrap_event_target(target);
}

/**
 * @param {EventTarget} target
 * @param {Event} event
 * @param {object=} targetOverride
 */
function dispatch(target, event, targetOverride) {
  op_event_dispatch(target, event, targetOverride);
}

/**
 * @param {EventTarget} target
 * @param {string} type
 * @return {EventListenerOrEventListenerObject[]}
 */
function getListeners(target, type) {
  return op_event_get_target_listeners(target, type);
}

/**
 * @param {EventTarget} target
 * @param {string} type
 * @returns {number}
 */
function getListenerCount(target, type) {
  return op_event_get_target_listener_count(target, type);
}

webidl.configureInterface(EventTarget);
const EventTargetPrototype = EventTarget.prototype;

ObjectDefineProperty(
  EventTarget.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    },
  },
);

defineEnumerableProps(EventTarget.prototype, [
  "addEventListener",
  "removeEventListener",
  "dispatchEvent",
]);

webidl.configureInterface(CustomEvent);
const CustomEventPrototype = CustomEvent.prototype;

ObjectDefineProperty(
  CustomEvent.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(CustomEventPrototype, this),
          keys: ArrayPrototypeFlat([
            EVENT_PROPS,
            "detail",
          ]),
        }),
        inspectOptions,
      );
    },
  },
);

ObjectDefineProperty(CustomEvent.prototype, "detail", {
  __proto__: null,
  enumerable: true,
});

webidl.configureInterface(ErrorEvent);
const ErrorEventPrototype = ErrorEvent.prototype;

ObjectDefineProperty(
  ErrorEvent.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(ErrorEventPrototype, this),
          keys: ArrayPrototypeFlat([
            EVENT_PROPS,
            ERROR_EVENT_PROPS,
          ]),
        }),
        inspectOptions,
      );
    },
  },
);

const ERROR_EVENT_PROPS = [
  "message",
  "filename",
  "lineno",
  "colno",
  "error",
];

defineEnumerableProps(ErrorEvent.prototype, ERROR_EVENT_PROPS);

webidl.configureInterface(CloseEvent);
const CloseEventPrototype = CloseEvent.prototype;

ObjectDefineProperty(
  CloseEvent.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(CloseEventPrototype, this),
          keys: ArrayPrototypeFlat([
            EVENT_PROPS,
            CLOSE_EVENT_PROPS,
          ]),
        }),
        inspectOptions,
      );
    },
  },
);

const CLOSE_EVENT_PROPS = [
  "wasClean",
  "code",
  "reason",
];

defineEnumerableProps(CloseEvent.prototype, CLOSE_EVENT_PROPS);

webidl.configureInterface(MessageEvent);
const MessageEventPrototype = MessageEvent.prototype;

ObjectDefineProperty(
  MessageEvent.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(MessageEventPrototype, this),
          keys: ArrayPrototypeFlat([
            EVENT_PROPS,
            MESSAGE_EVENT_PROPS,
          ]),
        }),
        inspectOptions,
      );
    },
  },
);

const MESSAGE_EVENT_PROPS = [
  "data",
  "origin",
  "lastEventId",
  "source",
  "ports",
];

defineEnumerableProps(MessageEvent.prototype, MESSAGE_EVENT_PROPS);

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
}

webidl.configureInterface(ProgressEvent);
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
    { bubbles, cancelable, composed, promise, reason } = { __proto__: null },
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
        keys: [...new SafeArrayIterator(EVENT_PROPS), "promise", "reason"],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(PromiseRejectionEvent);
const PromiseRejectionEventPrototype = PromiseRejectionEvent.prototype;

defineEnumerableProps(PromiseRejectionEvent.prototype, ["promise", "reason"]);

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
        handlerWrapper = makeWrappedHandler(value, isSpecialErrorEventHandler);
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
  EventTargetPrototype,
  getListenerCount,
  getListeners,
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
