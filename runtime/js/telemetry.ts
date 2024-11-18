// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_crypto_get_random_values,
  op_otel_instrumentation_scope_create_and_enter,
  op_otel_instrumentation_scope_enter,
  op_otel_instrumentation_scope_enter_builtin,
  op_otel_log,
  op_otel_span_attribute,
  op_otel_span_attribute2,
  op_otel_span_attribute3,
  op_otel_span_continue,
  op_otel_span_flush,
  op_otel_span_set_dropped,
  op_otel_span_start,
} from "ext:core/ops";
import { Console } from "ext:deno_console/01_console.js";
import { performance } from "ext:deno_web/15_performance.js";

const {
  SafeWeakMap,
  Array,
  ObjectEntries,
  SafeMap,
  ReflectApply,
  SymbolFor,
  Error,
  Uint8Array,
  TypedArrayPrototypeSubarray,
  ObjectAssign,
  ObjectDefineProperty,
  WeakRefPrototypeDeref,
  String,
  ObjectPrototypeIsPrototypeOf,
  DataView,
  DataViewPrototypeSetUint32,
  SafeWeakRef,
  TypedArrayPrototypeGetBuffer,
} = primordials;
const { AsyncVariable, setAsyncContext } = core;

let TRACING_ENABLED = false;
let DETERMINISTIC = false;

enum SpanKind {
  INTERNAL = 0,
  SERVER = 1,
  CLIENT = 2,
  PRODUCER = 3,
  CONSUMER = 4,
}

interface TraceState {
  set(key: string, value: string): TraceState;
  unset(key: string): TraceState;
  get(key: string): string | undefined;
  serialize(): string;
}

interface SpanContext {
  traceId: string;
  spanId: string;
  isRemote?: boolean;
  traceFlags: number;
  traceState?: TraceState;
}

type HrTime = [number, number];

enum SpanStatusCode {
  UNSET = 0,
  OK = 1,
  ERROR = 2,
}

interface SpanStatus {
  code: SpanStatusCode;
  message?: string;
}

export type AttributeValue =
  | string
  | number
  | boolean
  | Array<null | undefined | string>
  | Array<null | undefined | number>
  | Array<null | undefined | boolean>;

interface Attributes {
  [attributeKey: string]: AttributeValue | undefined;
}

type SpanAttributes = Attributes;

interface Link {
  context: SpanContext;
  attributes?: SpanAttributes;
  droppedAttributesCount?: number;
}

interface TimedEvent {
  time: HrTime;
  name: string;
  attributes?: SpanAttributes;
  droppedAttributesCount?: number;
}

interface IArrayValue {
  values: IAnyValue[];
}

interface IAnyValue {
  stringValue?: string | null;
  boolValue?: boolean | null;
  intValue?: number | null;
  doubleValue?: number | null;
  arrayValue?: IArrayValue;
  kvlistValue?: IKeyValueList;
  bytesValue?: Uint8Array;
}

interface IKeyValueList {
  values: IKeyValue[];
}

interface IKeyValue {
  key: string;
  value: IAnyValue;
}
interface IResource {
  attributes: IKeyValue[];
  droppedAttributesCount: number;
}

interface InstrumentationLibrary {
  readonly name: string;
  readonly version?: string;
  readonly schemaUrl?: string;
}

interface ReadableSpan {
  readonly name: string;
  readonly kind: SpanKind;
  readonly spanContext: () => SpanContext;
  readonly parentSpanId?: string;
  readonly startTime: HrTime;
  readonly endTime: HrTime;
  readonly status: SpanStatus;
  readonly attributes: SpanAttributes;
  readonly links: Link[];
  readonly events: TimedEvent[];
  readonly duration: HrTime;
  readonly ended: boolean;
  readonly resource: IResource;
  readonly instrumentationLibrary: InstrumentationLibrary;
  readonly droppedAttributesCount: number;
  readonly droppedEventsCount: number;
  readonly droppedLinksCount: number;
}

enum ExportResultCode {
  SUCCESS = 0,
  FAILED = 1,
}

interface ExportResult {
  code: ExportResultCode;
  error?: Error;
}

function hrToSecs(hr: [number, number]): number {
  return ((hr[0] * 1e3 + hr[1] / 1e6) / 1000);
}

const TRACE_FLAG_SAMPLED = 1 << 0;

const instrumentationScopes = new SafeWeakMap<
  InstrumentationLibrary,
  { __key: "instrumentation-library" }
>();
let activeInstrumentationLibrary: WeakRef<InstrumentationLibrary> | null = null;

function submit(
  spanId: string | Uint8Array,
  traceId: string | Uint8Array,
  traceFlags: number,
  parentSpanId: string | Uint8Array | null,
  span: Omit<
    ReadableSpan,
    | "spanContext"
    | "startTime"
    | "endTime"
    | "parentSpanId"
    | "duration"
    | "ended"
    | "resource"
  >,
  startTime: number,
  endTime: number,
) {
  if (!(traceFlags & TRACE_FLAG_SAMPLED)) return;

  // TODO(@lucacasonato): `resource` is ignored for now, should we implement it?

  const instrumentationLibrary = span.instrumentationLibrary;
  if (
    !activeInstrumentationLibrary ||
    WeakRefPrototypeDeref(activeInstrumentationLibrary) !==
      instrumentationLibrary
  ) {
    activeInstrumentationLibrary = new SafeWeakRef(instrumentationLibrary);
    if (instrumentationLibrary === BUILTIN_INSTRUMENTATION_LIBRARY) {
      op_otel_instrumentation_scope_enter_builtin();
    } else {
      let instrumentationScope = instrumentationScopes
        .get(instrumentationLibrary);

      if (instrumentationScope === undefined) {
        instrumentationScope = op_otel_instrumentation_scope_create_and_enter(
          instrumentationLibrary.name,
          instrumentationLibrary.version,
          instrumentationLibrary.schemaUrl,
        ) as { __key: "instrumentation-library" };
        instrumentationScopes.set(
          instrumentationLibrary,
          instrumentationScope,
        );
      } else {
        op_otel_instrumentation_scope_enter(
          instrumentationScope,
        );
      }
    }
  }

  op_otel_span_start(
    traceId,
    spanId,
    parentSpanId,
    span.kind,
    span.name,
    startTime,
    endTime,
  );

  const status = span.status;
  if (status !== null && status.code !== 0) {
    op_otel_span_continue(status.code, status.message ?? "");
  }

  const attributeKvs = ObjectEntries(span.attributes);
  let i = 0;
  while (i < attributeKvs.length) {
    if (i + 2 < attributeKvs.length) {
      op_otel_span_attribute3(
        attributeKvs.length,
        attributeKvs[i][0],
        attributeKvs[i][1],
        attributeKvs[i + 1][0],
        attributeKvs[i + 1][1],
        attributeKvs[i + 2][0],
        attributeKvs[i + 2][1],
      );
      i += 3;
    } else if (i + 1 < attributeKvs.length) {
      op_otel_span_attribute2(
        attributeKvs.length,
        attributeKvs[i][0],
        attributeKvs[i][1],
        attributeKvs[i + 1][0],
        attributeKvs[i + 1][1],
      );
      i += 2;
    } else {
      op_otel_span_attribute(
        attributeKvs.length,
        attributeKvs[i][0],
        attributeKvs[i][1],
      );
      i += 1;
    }
  }

  // TODO(@lucacasonato): implement links
  // TODO(@lucacasonato): implement events

  const droppedAttributesCount = span.droppedAttributesCount;
  const droppedLinksCount = span.droppedLinksCount + span.links.length;
  const droppedEventsCount = span.droppedEventsCount + span.events.length;
  if (
    droppedAttributesCount > 0 || droppedLinksCount > 0 ||
    droppedEventsCount > 0
  ) {
    op_otel_span_set_dropped(
      droppedAttributesCount,
      droppedLinksCount,
      droppedEventsCount,
    );
  }

  op_otel_span_flush();
}

const now = () => (performance.timeOrigin + performance.now()) / 1000;

const SPAN_ID_BYTES = 8;
const TRACE_ID_BYTES = 16;

const INVALID_TRACE_ID = new Uint8Array(TRACE_ID_BYTES);
const INVALID_SPAN_ID = new Uint8Array(SPAN_ID_BYTES);

const NO_ASYNC_CONTEXT = {};

let otelLog: (message: string, level: number) => void;

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

function bytesToHex(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 1) {
    out += hexSliceLookupTable[bytes[i]];
  }
  return out;
}

const SPAN_KEY = SymbolFor("OpenTelemetry Context Key SPAN");

const BUILTIN_INSTRUMENTATION_LIBRARY: InstrumentationLibrary = {} as never;

let COUNTER = 1;

export let enterSpan: (span: Span) => void;
export let exitSpan: (span: Span) => void;
export let endSpan: (span: Span) => void;

export class Span {
  #traceId: string | Uint8Array;
  #spanId: Uint8Array;
  #traceFlags = TRACE_FLAG_SAMPLED;

  #spanContext: SpanContext | null = null;

  #parentSpanId: string | Uint8Array | null = null;
  #parentSpanIdString: string | null = null;

  #recording = TRACING_ENABLED;

  #kind: number = 0;
  #name: string;
  #startTime: number;
  #status: { code: number; message?: string } | null = null;
  #attributes: Attributes = { __proto__: null } as never;

  #droppedEventsCount = 0;
  #droppedLinksCount = 0;

  #asyncContext = NO_ASYNC_CONTEXT;

  static {
    otelLog = function otelLog(message, level) {
      let traceId = null;
      let spanId = null;
      let traceFlags = 0;
      const span = CURRENT.get()?.getValue(SPAN_KEY);
      if (span) {
        // The lint is wrong, we can not use anything but `in` here because this
        // is a private field.
        // deno-lint-ignore prefer-primordials
        if (#traceId in span) {
          traceId = span.#traceId;
          spanId = span.#spanId;
          traceFlags = span.#traceFlags;
        } else {
          const context = span.spanContext();
          traceId = context.traceId;
          spanId = context.spanId;
          traceFlags = context.traceFlags;
        }
      }
      return op_otel_log(message, level, traceId, spanId, traceFlags);
    };

    enterSpan = (span: Span) => {
      if (!span.#recording) return;
      const context = (CURRENT.get() || ROOT_CONTEXT).setValue(SPAN_KEY, span);
      span.#asyncContext = CURRENT.enter(context);
    };

    exitSpan = (span: Span) => {
      if (!span.#recording) return;
      if (span.#asyncContext === NO_ASYNC_CONTEXT) return;
      setAsyncContext(span.#asyncContext);
      span.#asyncContext = NO_ASYNC_CONTEXT;
    };

    exitSpan = (span: Span) => {
      const endTime = now();
      submit(
        span.#spanId,
        span.#traceId,
        span.#traceFlags,
        span.#parentSpanId,
        {
          name: span.#name,
          kind: span.#kind,
          status: span.#status ?? { code: 0 },
          attributes: span.#attributes,
          events: [],
          links: [],
          droppedAttributesCount: 0,
          droppedEventsCount: span.#droppedEventsCount,
          droppedLinksCount: span.#droppedLinksCount,
          instrumentationLibrary: BUILTIN_INSTRUMENTATION_LIBRARY,
        },
        span.#startTime,
        endTime,
      );
    };
  }

  constructor(
    name: string,
    attributes?: Attributes,
  ) {
    if (!this.isRecording) {
      this.#name = "";
      this.#startTime = 0;
      this.#traceId = INVALID_TRACE_ID;
      this.#spanId = INVALID_SPAN_ID;
      this.#traceFlags = 0;
      return;
    }

    this.#name = name;
    this.#startTime = now();
    this.#attributes = attributes ?? { __proto__: null } as never;

    const currentSpan: Span | {
      spanContext(): { traceId: string; spanId: string };
    } = CURRENT.get()?.getValue(SPAN_KEY);
    if (!currentSpan) {
      const buffer = new Uint8Array(TRACE_ID_BYTES + SPAN_ID_BYTES);
      if (DETERMINISTIC) {
        DataViewPrototypeSetUint32(
          new DataView(TypedArrayPrototypeGetBuffer(buffer)),
          TRACE_ID_BYTES - 4,
          COUNTER,
          true,
        );
        COUNTER += 1;
        DataViewPrototypeSetUint32(
          new DataView(TypedArrayPrototypeGetBuffer(buffer)),
          TRACE_ID_BYTES + SPAN_ID_BYTES - 4,
          COUNTER,
          true,
        );
        COUNTER += 1;
      } else {
        op_crypto_get_random_values(buffer);
      }
      this.#traceId = TypedArrayPrototypeSubarray(buffer, 0, TRACE_ID_BYTES);
      this.#spanId = TypedArrayPrototypeSubarray(buffer, TRACE_ID_BYTES);
    } else {
      this.#spanId = new Uint8Array(SPAN_ID_BYTES);
      if (DETERMINISTIC) {
        DataViewPrototypeSetUint32(
          new DataView(TypedArrayPrototypeGetBuffer(this.#spanId)),
          SPAN_ID_BYTES - 4,
          COUNTER,
          true,
        );
        COUNTER += 1;
      } else {
        op_crypto_get_random_values(this.#spanId);
      }
      // deno-lint-ignore prefer-primordials
      if (#traceId in currentSpan) {
        this.#traceId = currentSpan.#traceId;
        this.#parentSpanId = currentSpan.#spanId;
      } else {
        const context = currentSpan.spanContext();
        this.#traceId = context.traceId;
        this.#parentSpanId = context.spanId;
      }
    }
  }

  spanContext() {
    if (!this.#spanContext) {
      this.#spanContext = {
        traceId: typeof this.#traceId === "string"
          ? this.#traceId
          : bytesToHex(this.#traceId),
        spanId: typeof this.#spanId === "string"
          ? this.#spanId
          : bytesToHex(this.#spanId),
        traceFlags: this.#traceFlags,
      };
    }
    return this.#spanContext;
  }

  get parentSpanId() {
    if (!this.#parentSpanIdString && this.#parentSpanId) {
      if (typeof this.#parentSpanId === "string") {
        this.#parentSpanIdString = this.#parentSpanId;
      } else {
        this.#parentSpanIdString = bytesToHex(this.#parentSpanId);
      }
    }
    return this.#parentSpanIdString;
  }

  setAttribute(name: string, value: AttributeValue) {
    if (this.#recording) this.#attributes[name] = value;
    return this;
  }

  setAttributes(attributes: Attributes) {
    if (this.#recording) ObjectAssign(this.#attributes, attributes);
    return this;
  }

  setStatus(status: { code: number; message?: string }) {
    if (this.#recording) {
      if (status.code === 0) {
        this.#status = null;
      } else if (status.code > 2) {
        throw new Error("Invalid status code");
      } else {
        this.#status = status;
      }
    }
    return this;
  }

  updateName(name: string) {
    if (this.#recording) this.#name = name;
    return this;
  }

  addEvent(_name: never) {
    // TODO(@lucacasonato): implement events
    if (this.#recording) this.#droppedEventsCount += 1;
    return this;
  }

  addLink(_link: never) {
    // TODO(@lucacasonato): implement links
    if (this.#recording) this.#droppedLinksCount += 1;
    return this;
  }

  addLinks(links: never[]) {
    // TODO(@lucacasonato): implement links
    if (this.#recording) this.#droppedLinksCount += links.length;
    return this;
  }

  isRecording() {
    return this.#recording;
  }
}

// Exporter compatible with opentelemetry js library
class SpanExporter {
  export(
    spans: ReadableSpan[],
    resultCallback: (result: ExportResult) => void,
  ) {
    try {
      for (let i = 0; i < spans.length; i += 1) {
        const span = spans[i];
        const context = span.spanContext();
        submit(
          context.spanId,
          context.traceId,
          context.traceFlags,
          span.parentSpanId ?? null,
          span,
          hrToSecs(span.startTime),
          hrToSecs(span.endTime),
        );
      }
      resultCallback({ code: 0 });
    } catch (error) {
      resultCallback({
        code: 1,
        error: ObjectPrototypeIsPrototypeOf(error, Error)
          ? error as Error
          : new Error(String(error)),
      });
    }
  }

  async shutdown() {}

  async forceFlush() {}
}

const CURRENT = new AsyncVariable();

class Context {
  #data = new SafeMap();

  // deno-lint-ignore no-explicit-any
  constructor(data?: Iterable<readonly [any, any]> | null | undefined) {
    this.#data = data ? new SafeMap(data) : new SafeMap();
  }

  getValue(key: symbol): unknown {
    return this.#data.get(key);
  }

  setValue(key: symbol, value: unknown): Context {
    const c = new Context(this.#data);
    c.#data.set(key, value);
    return c;
  }

  deleteValue(key: symbol): Context {
    const c = new Context(this.#data);
    c.#data.delete(key);
    return c;
  }
}

// TODO(lucacasonato): @opentelemetry/api defines it's own ROOT_CONTEXT
const ROOT_CONTEXT = new Context();

// Context manager for opentelemetry js library
class ContextManager {
  active(): Context {
    return CURRENT.get() ?? ROOT_CONTEXT;
  }

  with<A extends unknown[], F extends (...args: A) => ReturnType<F>>(
    context: Context,
    fn: F,
    thisArg?: ThisParameterType<F>,
    ...args: A
  ): ReturnType<F> {
    const ctx = CURRENT.enter(context);
    try {
      return ReflectApply(fn, thisArg, args);
    } finally {
      setAsyncContext(ctx);
    }
  }

  // deno-lint-ignore no-explicit-any
  bind<T extends (...args: any[]) => any>(
    context: Context,
    target: T,
  ): T {
    return ((...args) => {
      const ctx = CURRENT.enter(context);
      try {
        return ReflectApply(target, this, args);
      } finally {
        setAsyncContext(ctx);
      }
    }) as T;
  }

  enable() {
    return this;
  }

  disable() {
    return this;
  }
}

const otelConsoleConfig = {
  ignore: 0,
  capture: 1,
  replace: 2,
};

export function bootstrap(
  config: [] | [
    typeof otelConsoleConfig[keyof typeof otelConsoleConfig],
    number,
  ],
): void {
  if (config.length === 0) return;
  const { 0: consoleConfig, 1: deterministic } = config;

  TRACING_ENABLED = true;
  DETERMINISTIC = deterministic === 1;

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

export const telemetry = { SpanExporter, ContextManager };
