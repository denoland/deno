// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_crypto_get_random_values,
  op_otel_instrumentation_scope_create_and_enter,
  op_otel_instrumentation_scope_enter,
  op_otel_instrumentation_scope_enter_builtin,
  op_otel_log,
  op_otel_metric_attribute3,
  op_otel_metric_create_counter,
  op_otel_metric_create_gauge,
  op_otel_metric_create_histogram,
  op_otel_metric_create_observable_counter,
  op_otel_metric_create_observable_gauge,
  op_otel_metric_create_observable_up_down_counter,
  op_otel_metric_create_up_down_counter,
  op_otel_metric_observable_record0,
  op_otel_metric_observable_record1,
  op_otel_metric_observable_record2,
  op_otel_metric_observable_record3,
  op_otel_metric_observation_done,
  op_otel_metric_record0,
  op_otel_metric_record1,
  op_otel_metric_record2,
  op_otel_metric_record3,
  op_otel_metric_wait_to_observe,
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
  Array,
  ArrayPrototypePush,
  Error,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SafeIterator,
  SafeMap,
  SafePromiseAll,
  SafeSet,
  SafeWeakMap,
  SafeWeakRef,
  SafeWeakSet,
  String,
  StringPrototypePadStart,
  SymbolFor,
  TypedArrayPrototypeSubarray,
  Uint8Array,
  WeakRefPrototypeDeref,
} = primordials;
const { AsyncVariable, setAsyncContext } = core;

export let TRACING_ENABLED = false;
export let METRICS_ENABLED = false;
let DETERMINISTIC = false;

// Note: These start at 0 in the JS library,
// but start at 1 when serialized with JSON.
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

interface SpanOptions {
  attributes?: Attributes;
  kind?: SpanKind;
}

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

function activateInstrumentationLibrary(
  instrumentationLibrary: InstrumentationLibrary,
) {
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
}

function submitSpan(
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
  if (!TRACING_ENABLED) return;
  if (!(traceFlags & TRACE_FLAG_SAMPLED)) return;

  // TODO(@lucacasonato): `resource` is ignored for now, should we implement it?

  activateInstrumentationLibrary(span.instrumentationLibrary);

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
  #spanId: string | Uint8Array;
  #traceFlags = TRACE_FLAG_SAMPLED;

  #spanContext: SpanContext | null = null;

  #parentSpanId: string | Uint8Array | null = null;
  #parentSpanIdString: string | null = null;

  #recording = TRACING_ENABLED;

  #kind: number = SpanKind.INTERNAL;
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

    endSpan = (span: Span) => {
      const endTime = now();
      submitSpan(
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
    options?: SpanOptions,
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
    this.#attributes = options?.attributes ?? { __proto__: null } as never;
    this.#kind = options?.kind ?? SpanKind.INTERNAL;

    const currentSpan: Span | {
      spanContext(): { traceId: string; spanId: string };
    } = CURRENT.get()?.getValue(SPAN_KEY);
    if (currentSpan) {
      if (DETERMINISTIC) {
        this.#spanId = StringPrototypePadStart(String(COUNTER++), 16, "0");
      } else {
        this.#spanId = new Uint8Array(SPAN_ID_BYTES);
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
    } else {
      if (DETERMINISTIC) {
        this.#traceId = StringPrototypePadStart(String(COUNTER++), 32, "0");
        this.#spanId = StringPrototypePadStart(String(COUNTER++), 16, "0");
      } else {
        const buffer = new Uint8Array(TRACE_ID_BYTES + SPAN_ID_BYTES);
        op_crypto_get_random_values(buffer);
        this.#traceId = TypedArrayPrototypeSubarray(buffer, 0, TRACE_ID_BYTES);
        this.#spanId = TypedArrayPrototypeSubarray(buffer, TRACE_ID_BYTES);
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
        submitSpan(
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
  #data: Record<symbol, unknown> = { __proto__: null };

  constructor(data?: Record<symbol, unknown> | null | undefined) {
    this.#data = { __proto__: null, ...data };
  }

  getValue(key: symbol): unknown {
    return this.#data[key];
  }

  setValue(key: symbol, value: unknown): Context {
    const c = new Context(this.#data);
    c.#data[key] = value;
    return c;
  }

  deleteValue(key: symbol): Context {
    const c = new Context(this.#data);
    delete c.#data[key];
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

// metrics

interface MeterOptions {
  schemaUrl?: string;
}

interface MetricOptions {
  description?: string;

  unit?: string;

  valueType?: ValueType;

  advice?: MetricAdvice;
}

enum ValueType {
  INT = 0,
  DOUBLE = 1,
}

interface MetricAdvice {
  /**
   * Hint the explicit bucket boundaries for SDK if the metric is been
   * aggregated with a HistogramAggregator.
   */
  explicitBucketBoundaries?: number[];
}

export class MeterProvider {
  getMeter(name: string, version?: string, options?: MeterOptions): Meter {
    return new Meter({ name, version, schemaUrl: options?.schemaUrl });
  }
}

type MetricAttributes = Attributes;

type Instrument = { __key: "instrument" };

let batchResultHasObservables: (
  res: BatchObservableResult,
  observables: Observable[],
) => boolean;

class BatchObservableResult {
  #observables: WeakSet<Observable>;

  constructor(observables: WeakSet<Observable>) {
    this.#observables = observables;
  }

  static {
    batchResultHasObservables = (cb, observables) => {
      for (const observable of new SafeIterator(observables)) {
        if (!cb.#observables.has(observable)) return false;
      }
      return true;
    };
  }

  observe(
    metric: Observable,
    value: number,
    attributes?: MetricAttributes,
  ): void {
    if (!this.#observables.has(metric)) return;
    getObservableResult(metric).observe(value, attributes);
  }
}

const BATCH_CALLBACKS = new SafeMap<
  BatchObservableCallback,
  BatchObservableResult
>();
const INDIVIDUAL_CALLBACKS = new SafeMap<Observable, Set<ObservableCallback>>();

class Meter {
  #instrumentationLibrary: InstrumentationLibrary;

  constructor(instrumentationLibrary: InstrumentationLibrary) {
    this.#instrumentationLibrary = instrumentationLibrary;
  }

  createCounter(
    name: string,
    options?: MetricOptions,
  ): Counter {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Counter(null, false);
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_counter(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Counter(instrument, false);
  }

  createUpDownCounter(
    name: string,
    options?: MetricOptions,
  ): Counter {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Counter(null, true);
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_up_down_counter(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Counter(instrument, true);
  }

  createGauge(
    name: string,
    options?: MetricOptions,
  ): Gauge {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Gauge(null);
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_gauge(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Gauge(instrument);
  }

  createHistogram(
    name: string,
    options?: MetricOptions,
  ): Histogram {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Histogram(null);
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_histogram(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
      options?.advice?.explicitBucketBoundaries,
    ) as Instrument;
    return new Histogram(instrument);
  }

  createObservableCounter(
    name: string,
    options?: MetricOptions,
  ): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, true));
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_observable_counter(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Observable(new ObservableResult(instrument, true));
  }

  createObservableGauge(
    name: string,
    options?: MetricOptions,
  ): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, false));
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_observable_gauge(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Observable(new ObservableResult(instrument, false));
  }

  createObservableUpDownCounter(
    name: string,
    options?: MetricOptions,
  ): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, false));
    activateInstrumentationLibrary(this.#instrumentationLibrary);
    const instrument = op_otel_metric_create_observable_up_down_counter(
      name,
      // deno-lint-ignore prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Observable(new ObservableResult(instrument, false));
  }

  addBatchObservableCallback(
    callback: BatchObservableCallback,
    observables: Observable[],
  ): void {
    if (!METRICS_ENABLED) return;
    const result = new BatchObservableResult(new SafeWeakSet(observables));
    startObserving();
    BATCH_CALLBACKS.set(callback, result);
  }

  removeBatchObservableCallback(
    callback: BatchObservableCallback,
    observables: Observable[],
  ): void {
    if (!METRICS_ENABLED) return;
    const result = BATCH_CALLBACKS.get(callback);
    if (result && batchResultHasObservables(result, observables)) {
      BATCH_CALLBACKS.delete(callback);
    }
  }
}

type BatchObservableCallback = (
  observableResult: BatchObservableResult,
) => void | Promise<void>;

function record(
  instrument: Instrument | null,
  value: number,
  attributes?: MetricAttributes,
) {
  if (instrument === null) return;
  if (attributes === undefined) {
    op_otel_metric_record0(instrument, value);
  } else {
    const attrs = ObjectEntries(attributes);
    if (attrs.length === 0) {
      op_otel_metric_record0(instrument, value);
    }
    let i = 0;
    while (i < attrs.length) {
      const remaining = attrs.length - i;
      if (remaining > 3) {
        op_otel_metric_attribute3(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
          attrs[i + 2][0],
          attrs[i + 2][1],
        );
        i += 3;
      } else if (remaining === 3) {
        op_otel_metric_record3(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
          attrs[i + 2][0],
          attrs[i + 2][1],
        );
        i += 3;
      } else if (remaining === 2) {
        op_otel_metric_record2(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
        );
        i += 2;
      } else if (remaining === 1) {
        op_otel_metric_record1(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
        );
        i += 1;
      }
    }
  }
}

function recordObservable(
  instrument: Instrument | null,
  value: number,
  attributes?: MetricAttributes,
) {
  if (instrument === null) return;
  if (attributes === undefined) {
    op_otel_metric_observable_record0(instrument, value);
  } else {
    const attrs = ObjectEntries(attributes);
    if (attrs.length === 0) {
      op_otel_metric_observable_record0(instrument, value);
    }
    let i = 0;
    while (i < attrs.length) {
      const remaining = attrs.length - i;
      if (remaining > 3) {
        op_otel_metric_attribute3(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
          attrs[i + 2][0],
          attrs[i + 2][1],
        );
        i += 3;
      } else if (remaining === 3) {
        op_otel_metric_observable_record3(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
          attrs[i + 2][0],
          attrs[i + 2][1],
        );
        i += 3;
      } else if (remaining === 2) {
        op_otel_metric_observable_record2(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
          attrs[i + 1][0],
          attrs[i + 1][1],
        );
        i += 2;
      } else if (remaining === 1) {
        op_otel_metric_observable_record1(
          instrument,
          value,
          attrs[i][0],
          attrs[i][1],
        );
        i += 1;
      }
    }
  }
}

class Counter {
  #instrument: Instrument | null;
  #upDown: boolean;

  constructor(instrument: Instrument | null, upDown: boolean) {
    this.#instrument = instrument;
    this.#upDown = upDown;
  }

  add(value: number, attributes?: MetricAttributes, _context?: Context): void {
    if (value < 0 && !this.#upDown) {
      throw new Error("Counter can only be incremented");
    }
    record(this.#instrument, value, attributes);
  }
}

class Gauge {
  #instrument: Instrument | null;

  constructor(instrument: Instrument | null) {
    this.#instrument = instrument;
  }

  record(
    value: number,
    attributes?: MetricAttributes,
    _context?: Context,
  ): void {
    record(this.#instrument, value, attributes);
  }
}

class Histogram {
  #instrument: Instrument | null;

  constructor(instrument: Instrument | null) {
    this.#instrument = instrument;
  }

  record(
    value: number,
    attributes?: MetricAttributes,
    _context?: Context,
  ): void {
    record(this.#instrument, value, attributes);
  }
}

type ObservableCallback = (
  observableResult: ObservableResult,
) => void | Promise<void>;

let getObservableResult: (observable: Observable) => ObservableResult;

class Observable {
  #result: ObservableResult;

  constructor(result: ObservableResult) {
    this.#result = result;
  }

  static {
    getObservableResult = (observable) => observable.#result;
  }

  addCallback(callback: ObservableCallback): void {
    const res = INDIVIDUAL_CALLBACKS.get(this);
    if (res) res.add(callback);
    else INDIVIDUAL_CALLBACKS.set(this, new SafeSet([callback]));
    startObserving();
  }

  removeCallback(callback: ObservableCallback): void {
    const res = INDIVIDUAL_CALLBACKS.get(this);
    if (res) res.delete(callback);
    if (res?.size === 0) INDIVIDUAL_CALLBACKS.delete(this);
  }
}

class ObservableResult {
  #instrument: Instrument | null;
  #isRegularCounter: boolean;

  constructor(instrument: Instrument | null, isRegularCounter: boolean) {
    this.#instrument = instrument;
    this.#isRegularCounter = isRegularCounter;
  }

  observe(
    this: ObservableResult,
    value: number,
    attributes?: MetricAttributes,
  ): void {
    if (this.#isRegularCounter) {
      if (value < 0) {
        throw new Error("Observable counters can only be incremented");
      }
    }
    recordObservable(this.#instrument, value, attributes);
  }
}

async function observe(): Promise<void> {
  const promises: Promise<void>[] = [];
  // Primordials are not needed, because this is a SafeMap.
  // deno-lint-ignore prefer-primordials
  for (const { 0: observable, 1: callbacks } of INDIVIDUAL_CALLBACKS) {
    const result = getObservableResult(observable);
    // Primordials are not needed, because this is a SafeSet.
    // deno-lint-ignore prefer-primordials
    for (const callback of callbacks) {
      // PromiseTry is not in primordials?
      // deno-lint-ignore prefer-primordials
      ArrayPrototypePush(promises, Promise.try(callback, result));
    }
  }
  // Primordials are not needed, because this is a SafeMap.
  // deno-lint-ignore prefer-primordials
  for (const { 0: callback, 1: result } of BATCH_CALLBACKS) {
    // PromiseTry is not in primordials?
    // deno-lint-ignore prefer-primordials
    ArrayPrototypePush(promises, Promise.try(callback, result));
  }
  await SafePromiseAll(promises);
}

let isObserving = false;
function startObserving() {
  if (!isObserving) {
    isObserving = true;
    (async () => {
      while (true) {
        const promise = op_otel_metric_wait_to_observe();
        core.unrefOpPromise(promise);
        const ok = await promise;
        if (!ok) break;
        await observe();
        op_otel_metric_observation_done();
      }
    })();
  }
}

const otelConsoleConfig = {
  ignore: 0,
  capture: 1,
  replace: 2,
};

export function bootstrap(
  config: [
    0 | 1,
    0 | 1,
    typeof otelConsoleConfig[keyof typeof otelConsoleConfig],
    0 | 1,
  ],
): void {
  const {
    0: tracingEnabled,
    1: metricsEnabled,
    2: consoleConfig,
    3: deterministic,
  } = config;

  TRACING_ENABLED = tracingEnabled === 1;
  METRICS_ENABLED = metricsEnabled === 1;
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

export const telemetry = {
  SpanExporter,
  ContextManager,
  MeterProvider,
};
