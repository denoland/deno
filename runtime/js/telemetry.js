// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_otel_log,
  op_otel_span_attribute,
  op_otel_span_attribute2,
  op_otel_span_attribute3,
  op_otel_span_continue,
  op_otel_span_flush,
  op_otel_span_start,
} from "ext:core/ops";
import { Console } from "ext:deno_console/01_console.js";
import { performance } from "ext:deno_web/15_performance.js";

const {
  SymbolDispose,
  MathRandom,
  Array,
  ObjectEntries,
  SafeMap,
  ReflectApply,
  SymbolFor,
  Error,
} = primordials;
const { AsyncVariable, setAsyncContext } = core;

const CURRENT = new AsyncVariable();
let TRACING_ENABLED = false;

const SPAN_ID_BYTES = 8;
const TRACE_ID_BYTES = 16;

const TRACE_FLAG_SAMPLED = 1 << 0;

const hexSliceLookupTable = (function () {
  const alphabet = "0123456789abcdef";
  const table = new Array(256);
  for (let i = 0; i < 16; ++i) {
    const i16 = i * 16;
    for (let j = 0; j < 16; ++j) {
      table[i16 + j] = alphabet[i] + alphabet[j];
    }
  }
  return table;
})();

function generateId(bytes) {
  let out = "";
  for (let i = 0; i < bytes / 4; i += 1) {
    const r32 = (MathRandom() * 2 ** 32) >>> 0;
    out += hexSliceLookupTable[(r32 >> 24) & 0xff];
    out += hexSliceLookupTable[(r32 >> 16) & 0xff];
    out += hexSliceLookupTable[(r32 >> 8) & 0xff];
    out += hexSliceLookupTable[r32 & 0xff];
  }
  return out;
}

function submit(span) {
  if (!(span.traceFlags & TRACE_FLAG_SAMPLED)) return;

  op_otel_span_start(
    span.traceId,
    span.spanId,
    span.parentSpanId ?? "",
    span.kind,
    span.name,
    span.startTime,
    span.endTime,
  );

  if (span.status !== null && span.status.code !== 0) {
    op_otel_span_continue(span.code, span.message ?? "");
  }

  const attributes = ObjectEntries(span.attributes);
  let i = 0;
  while (i < attributes.length) {
    if (i + 2 < attributes.length) {
      op_otel_span_attribute3(
        attributes.length,
        attributes[i][0],
        attributes[i][1],
        attributes[i + 1][0],
        attributes[i + 1][1],
        attributes[i + 2][0],
        attributes[i + 2][1],
      );
      i += 3;
    } else if (i + 1 < attributes.length) {
      op_otel_span_attribute2(
        attributes.length,
        attributes[i][0],
        attributes[i][1],
        attributes[i + 1][0],
        attributes[i + 1][1],
      );
      i += 2;
    } else {
      op_otel_span_attribute(
        attributes.length,
        attributes[i][0],
        attributes[i][1],
      );
      i += 1;
    }
  }

  op_otel_span_flush();
}

const now = () => (performance.timeOrigin + performance.now()) / 1000;

const INVALID_SPAN_ID = "0000000000000000";
const INVALID_TRACE_ID = "00000000000000000000000000000000";
const NO_ASYNC_CONTEXT = {};

class Span {
  traceId;
  spanId;
  parentSpanId;
  kind;
  name;
  startTime;
  endTime;
  status = null;
  attributes = { __proto__: null };
  traceFlags = TRACE_FLAG_SAMPLED;

  enabled = TRACING_ENABLED;
  #asyncContext = NO_ASYNC_CONTEXT;

  constructor(name, kind = "internal") {
    if (!this.enabled) {
      this.traceId = INVALID_TRACE_ID;
      this.spanId = INVALID_SPAN_ID;
      this.parentSpanId = INVALID_SPAN_ID;
      return;
    }

    this.startTime = now();

    this.spanId = generateId(SPAN_ID_BYTES);

    let traceId;
    let parentSpanId;
    const parent = Span.current();
    if (parent) {
      if (parent.spanId !== undefined) {
        parentSpanId = parent.spanId;
        traceId = parent.traceId;
      } else {
        const context = parent.spanContext();
        parentSpanId = context.spanId;
        traceId = context.traceId;
      }
    }
    if (
      traceId && traceId !== INVALID_TRACE_ID && parentSpanId &&
      parentSpanId !== INVALID_SPAN_ID
    ) {
      this.traceId = traceId;
      this.parentSpanId = parentSpanId;
    } else {
      this.traceId = generateId(TRACE_ID_BYTES);
      this.parentSpanId = INVALID_SPAN_ID;
    }

    this.name = name;

    switch (kind) {
      case "internal":
        this.kind = 0;
        break;
      case "server":
        this.kind = 1;
        break;
      case "client":
        this.kind = 2;
        break;
      case "producer":
        this.kind = 3;
        break;
      case "consumer":
        this.kind = 4;
        break;
      default:
        throw new Error(`Invalid span kind: ${kind}`);
    }

    this.enter();
  }

  // helper function to match otel js api
  spanContext() {
    return {
      traceId: this.traceId,
      spanId: this.spanId,
      traceFlags: this.traceFlags,
    };
  }

  setAttribute(name, value) {
    if (!this.enabled) return;
    this.attributes[name] = value;
  }

  enter() {
    if (!this.enabled) return;
    const context = (CURRENT.get() || ROOT_CONTEXT).setValue(SPAN_KEY, this);
    this.#asyncContext = CURRENT.enter(context);
  }

  exit() {
    if (!this.enabled || this.#asyncContext === NO_ASYNC_CONTEXT) return;
    setAsyncContext(this.#asyncContext);
    this.#asyncContext = NO_ASYNC_CONTEXT;
  }

  end() {
    if (!this.enabled || this.endTime !== undefined) return;
    this.exit();
    this.endTime = now();
    submit(this);
  }

  [SymbolDispose]() {
    this.end();
  }

  static current() {
    return CURRENT.get()?.getValue(SPAN_KEY);
  }
}

function hrToSecs(hr) {
  return ((hr[0] * 1e3 + hr[1] / 1e6) / 1000);
}

// Exporter compatible with opentelemetry js library
class SpanExporter {
  export(spans, resultCallback) {
    try {
      for (let i = 0; i < spans.length; i += 1) {
        const span = spans[i];
        const context = span.spanContext();
        submit({
          spanId: context.spanId,
          traceId: context.traceId,
          traceFlags: context.traceFlags,
          name: span.name,
          kind: span.kind,
          parentSpanId: span.parentSpanId,
          startTime: hrToSecs(span.startTime),
          endTime: hrToSecs(span.endTime),
          status: span.status,
          attributes: span.attributes,
        });
      }
      resultCallback({ code: 0 });
    } catch (error) {
      resultCallback({ code: 1, error });
    }
  }

  async shutdown() {}

  async forceFlush() {}
}

// SPAN_KEY matches symbol in otel-js library
const SPAN_KEY = SymbolFor("OpenTelemetry Context Key SPAN");

// Context tracker compatible with otel-js api
class Context {
  #data = new SafeMap();

  constructor(data) {
    this.#data = data ? new SafeMap(data) : new SafeMap();
  }

  getValue(key) {
    return this.#data.get(key);
  }

  setValue(key, value) {
    const c = new Context(this.#data);
    c.#data.set(key, value);
    return c;
  }

  deleteValue(key) {
    const c = new Context(this.#data);
    c.#data.delete(key);
    return c;
  }
}

const ROOT_CONTEXT = new Context();

// Context manager for opentelemetry js library
class ContextManager {
  active() {
    return CURRENT.get() ?? ROOT_CONTEXT;
  }

  with(context, fn, thisArg, ...args) {
    const ctx = CURRENT.enter(context);
    try {
      return ReflectApply(fn, thisArg, args);
    } finally {
      setAsyncContext(ctx);
    }
  }

  bind(context, f) {
    return (...args) => {
      const ctx = CURRENT.enter(context);
      try {
        return ReflectApply(f, thisArg, args);
      } finally {
        setAsyncContext(ctx);
      }
    };
  }

  enable() {
    return this;
  }

  disable() {
    return this;
  }
}

function otelLog(message, level) {
  let traceId = "";
  let spanId = "";
  let traceFlags = 0;
  const span = Span.current();
  if (span) {
    if (span.spanId !== undefined) {
      spanId = span.spanId;
      traceId = span.traceId;
      traceFlags = span.traceFlags;
    } else {
      const context = span.spanContext();
      spanId = context.spanId;
      traceId = context.traceId;
      traceFlags = context.traceFlags;
    }
  }
  return op_otel_log(message, level, traceId, spanId, traceFlags);
}

const otelConsoleConfig = {
  ignore: 0,
  capture: 1,
  replace: 2,
};

export function bootstrap(config) {
  if (config.length === 0) return;
  const { 0: consoleConfig } = config;

  TRACING_ENABLED = true;

  switch (consoleConfig) {
    case otelConsoleConfig.capture:
      core.wrapConsole(globalThis.console, new Console(otelLog));
      break;
    case otelConsoleConfig.replace:
      ObjectDefineProperty(
        globalThis,
        "console",
        core.propNonEnumerable(new Console(otelLog)),
      );
      break;
    default:
      break;
  }
}

export const tracing = {
  get enabled() {
    return TRACING_ENABLED;
  },
  Span,
  SpanExporter,
  ContextManager,
};

// TODO(devsnek): implement metrics
export const metrics = {};
