// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_otel_collect_isolate_metrics,
  op_otel_enable_isolate_metrics,
  op_otel_log,
  op_otel_log_foreign,
  op_otel_metric_attribute3,
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
  op_otel_baggage_parse,
  op_otel_baggage_serialize,
  op_otel_parse_traceparent,
  op_otel_span_context_valid,
  op_otel_span_add_link,
  op_otel_span_attribute1,
  op_otel_span_attribute2,
  op_otel_span_attribute3,
  op_otel_span_update_name,
  OtelMeter,
  OtelTracer,
  OtelTraceState,
} = core.ops;
const { Console } = core.loadExtScript("ext:deno_web/01_console.js");

const {
  ArrayFrom,
  ArrayIsArray,
  ArrayPrototypeConcat,
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeReduce,
  ArrayPrototypeSlice,
  DatePrototype,
  DatePrototypeGetTime,
  Error,
  MapPrototypeEntries,
  Number,
  NumberPrototypeToString,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ObjectValues,
  ReflectApply,
  SafeArrayIterator,
  SafeMap,
  SafeMapIterator,
  SafePromiseAll,
  SafeSet,
  SafeWeakSet,
  SymbolFor,
  TypeError,
} = primordials;
const { AsyncVariable, getAsyncContext, setAsyncContext } = core;

let TRACING_ENABLED = false;
let METRICS_ENABLED = false;
let PROPAGATORS: TextMapPropagator[] = [];
let ISOLATE_METRICS = false;

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

// Rust-backed `TraceState` implementation (see ext/telemetry/propagation.rs).
// Parsing, validation, member storage, clone/set/unset semantics and
// serialization all live in Rust; this is just the cppgc object's type.
interface OtelTraceState extends TraceState {
  __key: "traceState";

  // deno-lint-ignore no-misused-new
  new (rawTraceState?: string): OtelTraceState;

  set(key: string, value: string): OtelTraceState;
  unset(key: string): OtelTraceState;
}

interface SpanContext {
  traceId: string;
  spanId: string;
  isRemote?: boolean;
  traceFlags: number;
  traceState?: TraceState;
}

enum SpanStatusCode {
  UNSET = 0,
  OK = 1,
  ERROR = 2,
}

interface SpanStatus {
  code: SpanStatusCode;
  message?: string;
}

type AttributeValue =
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

interface Exception {
  code?: string | number;
  message?: string;
  name?: string;
  stack?: string;
}

type TimeInput = [number, number] | number | Date;

interface SpanOptions {
  kind?: SpanKind;
  attributes?: Attributes;
  links?: Link[];
  startTime?: TimeInput;
  root?: boolean;
}

interface Link {
  context: SpanContext;
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

function hrToMs(hr: [number, number]): number {
  return (hr[0] * 1e3 + hr[1] / 1e6);
}

function isTimeInput(input: unknown): input is TimeInput {
  return typeof input === "number" ||
    (input && (ArrayIsArray(input) || isDate(input)));
}

function timeInputToMs(input?: TimeInput): number | undefined {
  if (input === undefined) return;
  if (ArrayIsArray(input)) {
    return hrToMs(input);
  } else if (isDate(input)) {
    return DatePrototypeGetTime(input);
  }
  return input;
}

function countAttributes(attributes?: Attributes): number {
  return attributes ? ObjectKeys(attributes).length : 0;
}

interface AsyncContextSnapshot {
  __brand: "AsyncContextSnapshot";
}

function enterSpan(
  span: Span,
  context?: Context,
): AsyncContextSnapshot | undefined {
  if (!span.isRecording()) return undefined;
  context = (context ?? CURRENT.get() ?? ROOT_CONTEXT)
    .setValue(SPAN_KEY, span);
  return CURRENT.enter(context);
}

const currentSnapshot = getAsyncContext;
const restoreSnapshot = setAsyncContext;

function isDate(value: unknown): value is Date {
  return ObjectPrototypeIsPrototypeOf(DatePrototype, value);
}

interface OtelTracer {
  __key: "tracer";

  // deno-lint-ignore no-misused-new
  new (name: string, version?: string, schemaUrl?: string): OtelTracer;

  startSpan(
    parent: OtelSpan | undefined,
    name: string,
    spanKind: SpanKind,
    startTime: number | undefined,
    attributeCount: number,
  ): OtelSpan;

  startSpanForeign(
    parentTraceId: string,
    parentSpanId: string,
    parentTraceFlags: number,
    name: string,
    spanKind: SpanKind,
    startTime: number | undefined,
    attributeCount: number,
  ): OtelSpan;
}

interface OtelSpan {
  __key: "span";

  spanContext(): SpanContext;
  setStatus(status: SpanStatusCode, errorDescription: string): void;
  addEvent(
    name: string,
    startTime: number,
  ): number;
  dropEvent(): void;
  end(endTime: number): void;
}

enum SpanAttributesLocation {
  SELF = 0,
  EVENT = 1,
  LINK = 2,
}

function spanAddAttributes(
  span: OtelSpan,
  attributesLocation: SpanAttributesLocation,
  attributesTarget: number,
  attributes: Attributes,
) {
  const attributeKvs = ObjectEntries(attributes);
  let i = 0;
  while (i < attributeKvs.length) {
    if (i + 2 < attributeKvs.length) {
      op_otel_span_attribute3(
        span,
        attributesLocation,
        attributesTarget,
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
        span,
        attributesLocation,
        attributesTarget,
        attributeKvs[i][0],
        attributeKvs[i][1],
        attributeKvs[i + 1][0],
        attributeKvs[i + 1][1],
      );
      i += 2;
    } else {
      op_otel_span_attribute1(
        span,
        attributesLocation,
        attributesTarget,
        attributeKvs[i][0],
        attributeKvs[i][1],
      );
      i += 1;
    }
  }
}

interface TracerOptions {
  schemaUrl?: string;
}

class TracerProvider {
  constructor() {
    throw new TypeError("TracerProvider can not be constructed");
  }

  static getTracer(
    name: string,
    version?: string,
    options?: TracerOptions,
  ): Tracer {
    const tracer = new OtelTracer(name, version, options?.schemaUrl);
    return new Tracer(tracer);
  }
}

class Tracer {
  #tracer: OtelTracer;

  constructor(tracer: OtelTracer) {
    this.#tracer = tracer;
  }

  startActiveSpan<F extends (span: Span) => unknown>(
    name: string,
    fn: F,
  ): ReturnType<F>;
  startActiveSpan<F extends (span: Span) => unknown>(
    name: string,
    options: SpanOptions,
    fn: F,
  ): ReturnType<F>;
  startActiveSpan<F extends (span: Span) => unknown>(
    name: string,
    options: SpanOptions,
    context: Context,
    fn: F,
  ): ReturnType<F>;
  startActiveSpan<F extends (span: Span) => unknown>(
    name: string,
    optionsOrFn: SpanOptions | F,
    fnOrContext?: F | Context,
    maybeFn?: F,
  ) {
    let options;
    let context;
    let fn;
    if (typeof optionsOrFn === "function") {
      options = undefined;
      fn = optionsOrFn;
    } else if (typeof fnOrContext === "function") {
      options = optionsOrFn;
      fn = fnOrContext;
    } else if (typeof maybeFn === "function") {
      options = optionsOrFn;
      context = fnOrContext;
      fn = maybeFn;
    } else {
      throw new Error("startActiveSpan requires a function argument");
    }
    if (options?.root) {
      context = ROOT_CONTEXT;
    } else {
      context = context ?? CURRENT.get() ?? ROOT_CONTEXT;
    }
    const span = this.startSpan(name, options, context);
    const ctx = CURRENT.enter(context.setValue(SPAN_KEY, span));
    try {
      return ReflectApply(fn, undefined, [span]);
    } finally {
      setAsyncContext(ctx);
    }
  }

  startSpan(name: string, options?: SpanOptions, context?: Context): Span {
    if (options?.root) {
      context = undefined;
    } else {
      context = context ?? CURRENT.get();
    }

    const startTime = timeInputToMs(options?.startTime);

    const parentSpan = context?.getValue(SPAN_KEY) as
      | Span
      | { spanContext(): SpanContext }
      | undefined;
    const attributesCount = countAttributes(options?.attributes);
    const parentOtelSpan: OtelSpan | null | undefined = parentSpan !== undefined
      ? getOtelSpan(parentSpan) ?? undefined
      : undefined;
    let otelSpan: OtelSpan;
    if (parentOtelSpan || !parentSpan) {
      otelSpan = this.#tracer.startSpan(
        parentOtelSpan,
        name,
        options?.kind ?? 0,
        startTime,
        attributesCount,
      );
    } else {
      const spanContext = parentSpan.spanContext();
      otelSpan = this.#tracer.startSpanForeign(
        spanContext.traceId,
        spanContext.spanId,
        spanContext.traceFlags ?? 0,
        name,
        options?.kind ?? 0,
        startTime,
        attributesCount,
      );
    }
    const span = new Span(otelSpan);
    if (options?.links) span.addLinks(options?.links);
    if (options?.attributes) span.setAttributes(options?.attributes);
    return span;
  }
}

const SPAN_KEY = SymbolFor("OpenTelemetry Context Key SPAN");

let getOtelSpan: (span: object) => OtelSpan | null | undefined;

class Span {
  #otelSpan: OtelSpan | null;
  #spanContext: SpanContext | undefined;

  static {
    getOtelSpan = (span) => (#otelSpan in span ? span.#otelSpan : undefined);
  }

  constructor(otelSpan: OtelSpan | null) {
    this.#otelSpan = otelSpan;
  }

  spanContext() {
    if (!this.#spanContext) {
      if (this.#otelSpan) {
        this.#spanContext = this.#otelSpan.spanContext();
      } else {
        this.#spanContext = {
          traceId: "00000000000000000000000000000000",
          spanId: "0000000000000000",
          traceFlags: 0,
        };
      }
    }
    return this.#spanContext;
  }

  addEvent(
    name: string,
    attributesOrStartTime?: Attributes | TimeInput,
    startTime?: TimeInput,
  ): this {
    if (!this.#otelSpan) return this;
    let attributes: Attributes | undefined;
    if (isTimeInput(attributesOrStartTime)) {
      startTime = attributesOrStartTime;
    } else {
      attributes = attributesOrStartTime;
    }
    const startTimeMs = timeInputToMs(startTime);

    const attributesTarget = this.#otelSpan.addEvent(
      name,
      startTimeMs ?? NaN,
    );
    if (attributes && attributesTarget !== 0) {
      spanAddAttributes(
        this.#otelSpan,
        SpanAttributesLocation.EVENT,
        attributesTarget,
        attributes,
      );
    }
    return this;
  }

  addLink(link: Link): this {
    if (!this.#otelSpan) return this;
    const attributesTarget = op_otel_span_add_link(
      this.#otelSpan,
      link.context.traceId,
      link.context.spanId,
      link.context.traceFlags,
      link.context.isRemote ?? false,
      link.droppedAttributesCount ?? 0,
    );
    if (link.attributes && attributesTarget !== 0) {
      spanAddAttributes(
        this.#otelSpan,
        SpanAttributesLocation.LINK,
        attributesTarget,
        link.attributes,
      );
    }
    return this;
  }

  addLinks(links: Link[]): this {
    for (let i = 0; i < links.length; i++) {
      this.addLink(links[i]);
    }
    return this;
  }

  end(endTime?: TimeInput): void {
    this.#otelSpan?.end(timeInputToMs(endTime) || NaN);
  }

  isRecording(): boolean {
    return this.#otelSpan !== undefined;
  }

  recordException(exception: string | Exception, time?: TimeInput): void {
    if (typeof exception === "string") {
      this.addEvent("exception", {
        "exception.message": exception,
      }, time);
      return;
    }
    const attributes: Attributes = {};

    if (exception.code) {
      if (typeof exception.code === "number") {
        attributes["exception.type"] = NumberPrototypeToString(exception.code);
      } else {
        attributes["exception.type"] = exception.code;
      }
    } else if (exception.name) {
      attributes["exception.type"] = exception.name;
    }

    if (exception.message) {
      attributes["exception.message"] = exception.message;
    }
    if (exception.stack) {
      attributes["exception.stacktrace"] = exception.stack;
    }

    this.addEvent("exception", attributes, time);
  }

  setAttribute(key: string, value: AttributeValue): this {
    if (!this.#otelSpan) return this;
    op_otel_span_attribute1(
      this.#otelSpan,
      SpanAttributesLocation.SELF,
      0,
      key,
      value,
    );
    return this;
  }

  setAttributes(attributes: Attributes): this {
    if (!this.#otelSpan) return this;
    spanAddAttributes(
      this.#otelSpan,
      SpanAttributesLocation.SELF,
      0,
      attributes,
    );
    return this;
  }

  setStatus(status: SpanStatus): this {
    this.#otelSpan?.setStatus(status.code, status.message ?? "");
    return this;
  }

  updateName(name: string): this {
    if (!this.#otelSpan) return this;
    op_otel_span_update_name(this.#otelSpan, name);
    return this;
  }
}

const CURRENT = new AsyncVariable();

class Context {
  // @ts-ignore __proto__ is not supported in TypeScript
  #data: Record<symbol, unknown> = { __proto__: null };

  constructor(data?: Record<symbol, unknown> | null | undefined) {
    // @ts-ignore __proto__ is not supported in TypeScript
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
  constructor() {
    throw new TypeError("ContextManager can not be constructed");
  }

  static active(): Context {
    return CURRENT.get() ?? ROOT_CONTEXT;
  }

  static with<A extends unknown[], F extends (...args: A) => ReturnType<F>>(
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
  static bind<T extends (...args: any[]) => any>(
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

  static enable() {
    return this;
  }

  static disable() {
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

interface OtelMeter {
  __key: "meter";
  createCounter(name: string, description?: string, unit?: string): Instrument;
  createUpDownCounter(
    name: string,
    description?: string,
    unit?: string,
  ): Instrument;
  createGauge(name: string, description?: string, unit?: string): Instrument;
  createHistogram(
    name: string,
    description?: string,
    unit?: string,
    explicitBucketBoundaries?: number[],
  ): Instrument;
  createObservableCounter(
    name: string,
    description?: string,
    unit?: string,
  ): Instrument;
  createObservableUpDownCounter(
    name: string,
    description?: string,
    unit?: string,
  ): Instrument;
  createObservableGauge(
    name: string,
    description?: string,
    unit?: string,
  ): Instrument;
}

class MeterProvider {
  constructor() {
    throw new TypeError("MeterProvider can not be constructed");
  }

  static getMeter(
    name: string,
    version?: string,
    options?: MeterOptions,
  ): Meter {
    const meter = new OtelMeter(name, version, options?.schemaUrl);
    return new Meter(meter);
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
      for (const observable of new SafeArrayIterator(observables)) {
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
  #meter: OtelMeter;

  constructor(meter: OtelMeter) {
    this.#meter = meter;
  }

  createCounter(name: string, options?: MetricOptions): Counter {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Counter(null, false);
    const instrument = this.#meter.createCounter(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Counter(instrument, false);
  }

  createUpDownCounter(name: string, options?: MetricOptions): Counter {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Counter(null, true);
    const instrument = this.#meter.createUpDownCounter(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Counter(instrument, true);
  }

  createGauge(name: string, options?: MetricOptions): Gauge {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Gauge(null);
    const instrument = this.#meter.createGauge(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Gauge(instrument);
  }

  createHistogram(name: string, options?: MetricOptions): Histogram {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) return new Histogram(null);
    const instrument = this.#meter.createHistogram(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
      options?.advice?.explicitBucketBoundaries,
    ) as Instrument;
    return new Histogram(instrument);
  }

  createObservableCounter(name: string, options?: MetricOptions): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, true));
    const instrument = this.#meter.createObservableCounter(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Observable(new ObservableResult(instrument, true));
  }

  createObservableUpDownCounter(
    name: string,
    options?: MetricOptions,
  ): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, false));
    const instrument = this.#meter.createObservableUpDownCounter(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
      options?.description,
      options?.unit,
    ) as Instrument;
    return new Observable(new ObservableResult(instrument, false));
  }

  createObservableGauge(name: string, options?: MetricOptions): Observable {
    if (options?.valueType !== undefined && options?.valueType !== 1) {
      throw new Error("Only valueType: DOUBLE is supported");
    }
    if (!METRICS_ENABLED) new Observable(new ObservableResult(null, false));
    const instrument = this.#meter.createObservableGauge(
      name,
      // deno-lint-ignore deno-internal/prefer-primordials
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
          attrs.length,
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
        op_otel_metric_record1(instrument, value, attrs[i][0], attrs[i][1]);
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
          attrs.length,
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
  if (ISOLATE_METRICS) {
    op_otel_collect_isolate_metrics();
  }

  const promises: Promise<void>[] = [];
  // Primordials are not needed, because this is a SafeMap.
  // deno-lint-ignore deno-internal/prefer-primordials
  for (const { 0: observable, 1: callbacks } of INDIVIDUAL_CALLBACKS) {
    const result = getObservableResult(observable);
    // Primordials are not needed, because this is a SafeSet.
    // deno-lint-ignore deno-internal/prefer-primordials
    for (const callback of callbacks) {
      // PromiseTry is not in primordials?
      // deno-lint-ignore deno-internal/prefer-primordials
      ArrayPrototypePush(promises, Promise.try(callback, result));
    }
  }
  // Primordials are not needed, because this is a SafeMap.
  // deno-lint-ignore deno-internal/prefer-primordials
  for (const { 0: callback, 1: result } of BATCH_CALLBACKS) {
    // PromiseTry is not in primordials?
    // deno-lint-ignore deno-internal/prefer-primordials
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

let pendingException: Error | undefined;

const OTEL_CONSOLE_METHODS = ["log", "debug", "info", "warn", "error"];

function wrapOtelConsoleMethods(otelConsole: Console) {
  for (let i = 0; i < OTEL_CONSOLE_METHODS.length; i++) {
    const method = OTEL_CONSOLE_METHODS[i];
    const orig = otelConsole[method];
    otelConsole[method] = (...args: unknown[]) => {
      if (args.length >= 1 && core.isNativeError(args[0])) {
        pendingException = args[0] as Error;
      }
      return ReflectApply(orig, otelConsole, args);
    };
  }
}

function otelLog(message: string, level: number) {
  const exception = pendingException;
  pendingException = undefined;
  const excType = exception?.name ?? "";
  const excMessage = exception?.message ?? "";
  const excStacktrace = exception?.stack ?? "";
  const currentSpan = CURRENT.get()?.getValue(SPAN_KEY);
  const otelSpan = currentSpan !== undefined
    ? getOtelSpan(currentSpan)
    : undefined;
  if (otelSpan || currentSpan === undefined) {
    op_otel_log(message, level, otelSpan, excType, excMessage, excStacktrace);
  } else {
    const spanContext = currentSpan.spanContext();
    op_otel_log_foreign(
      message,
      level,
      spanContext.traceId,
      spanContext.spanId,
      spanContext.traceFlags,
      excType,
      excMessage,
      excStacktrace,
    );
  }
}

/*
 * Copyright The OpenTelemetry Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// The trace-context version we emit when injecting `traceparent`. Parsing,
// validation, the `TraceState` object and trace-state/baggage (de)serialization
// now live in Rust (see ext/telemetry/propagation.rs).
const VERSION = "00";

const TRACE_PARENT_HEADER = "traceparent";
const TRACE_STATE_HEADER = "tracestate";
const INVALID_TRACEID = "00000000000000000000000000000000";
const INVALID_SPANID = "0000000000000000";
const INVALID_SPAN_CONTEXT: SpanContext = {
  traceId: INVALID_TRACEID,
  spanId: INVALID_SPANID,
  traceFlags: 0,
};
const BAGGAGE_ITEMS_SEPARATOR = ",";
const BAGGAGE_HEADER = "baggage";

class NonRecordingSpan implements Span {
  constructor(
    private readonly _spanContext: SpanContext = INVALID_SPAN_CONTEXT,
  ) {}

  spanContext(): SpanContext {
    return this._spanContext;
  }

  setAttribute(_key: string, _value: unknown): this {
    return this;
  }

  setAttributes(_attributes: SpanAttributes): this {
    return this;
  }

  addEvent(_name: string, _attributes?: SpanAttributes): this {
    return this;
  }

  addLink(_link: Link): this {
    return this;
  }

  addLinks(_links: Link[]): this {
    return this;
  }

  setStatus(_status: SpanStatus): this {
    return this;
  }

  updateName(_name: string): this {
    return this;
  }

  end(_endTime?: TimeInput): void {}

  isRecording(): boolean {
    return false;
  }

  // deno-lint-ignore no-explicit-any
  recordException(_exception: any, _time?: TimeInput): void {}
}

const otelPropagators = {
  traceContext: 0,
  baggage: 1,
  none: 2,
};

function parseTraceParent(traceParent: string): SpanContext | null {
  // Parsing and validation (including the version/future-version rules) is
  // performed in Rust. The op returns `{ traceId, spanId, traceFlags }` or
  // null.
  return op_otel_parse_traceparent(traceParent);
}

// deno-lint-ignore no-explicit-any
interface TextMapSetter<Carrier = any> {
  set(carrier: Carrier, key: string, value: string): void;
}

// deno-lint-ignore no-explicit-any
interface TextMapPropagator<Carrier = any> {
  inject(
    context: Context,
    carrier: Carrier,
    setter: TextMapSetter<Carrier>,
  ): void;
  extract(
    context: Context,
    carrier: Carrier,
    getter: TextMapGetter<Carrier>,
  ): Context;
  fields(): string[];
}

// deno-lint-ignore no-explicit-any
interface TextMapGetter<Carrier = any> {
  keys(carrier: Carrier): string[];
  get(carrier: Carrier, key: string): undefined | string | string[];
}

function isTracingSuppressed(context: Context): boolean {
  return context.getValue(
    SymbolFor("OpenTelemetry SDK Context Key SUPPRESS_TRACING"),
  ) === true;
}

function isSpanContextValid(spanContext: SpanContext): boolean {
  // Trace id / span id validation is performed in Rust.
  return op_otel_span_context_valid(spanContext.traceId, spanContext.spanId);
}

class W3CTraceContextPropagator implements TextMapPropagator {
  inject(context: Context, carrier: unknown, setter: TextMapSetter): void {
    const spanContext = (context.getValue(SPAN_KEY) as Span | undefined)
      ?.spanContext();
    if (
      !spanContext ||
      isTracingSuppressed(context) ||
      !isSpanContextValid(spanContext)
    ) {
      return;
    }

    const traceParent =
      `${VERSION}-${spanContext.traceId}-${spanContext.spanId}-0${
        NumberPrototypeToString(Number(spanContext.traceFlags || 0), 16)
      }`;

    setter.set(carrier, TRACE_PARENT_HEADER, traceParent);
    if (spanContext.traceState) {
      setter.set(
        carrier,
        TRACE_STATE_HEADER,
        spanContext.traceState.serialize(),
      );
    }
  }

  extract(context: Context, carrier: unknown, getter: TextMapGetter): Context {
    const traceParentHeader = getter.get(carrier, TRACE_PARENT_HEADER);
    if (!traceParentHeader) return context;
    const traceParent = ArrayIsArray(traceParentHeader)
      ? traceParentHeader[0]
      : traceParentHeader;
    if (typeof traceParent !== "string") return context;
    const spanContext = parseTraceParent(traceParent);
    if (!spanContext) return context;

    spanContext.isRemote = true;

    const traceStateHeader = getter.get(carrier, TRACE_STATE_HEADER);
    if (traceStateHeader) {
      // If more than one `tracestate` header is found, we merge them into a
      // single header.
      const state = ArrayIsArray(traceStateHeader)
        ? ArrayPrototypeJoin(traceStateHeader, ",")
        : traceStateHeader;
      spanContext.traceState = new OtelTraceState(
        typeof state === "string" ? state : undefined,
      );
    }
    return context.setValue(SPAN_KEY, new NonRecordingSpan(spanContext));
  }

  fields(): string[] {
    return [TRACE_PARENT_HEADER, TRACE_STATE_HEADER];
  }
}

const baggageEntryMetadataSymbol = SymbolFor("BaggageEntryMetadata");

type BaggageEntryMetadata = { toString(): string } & {
  __TYPE__: typeof baggageEntryMetadataSymbol;
};

interface BaggageEntry {
  value: string;
  metadata?: BaggageEntryMetadata;
}

interface Baggage {
  getEntry(key: string): BaggageEntry | undefined;
  getAllEntries(): [string, BaggageEntry][];
  setEntry(key: string, entry: BaggageEntry): Baggage;
  removeEntry(key: string): Baggage;
  removeEntries(...key: string[]): Baggage;
  clear(): Baggage;
}

function baggageEntryMetadataFromString(
  str: string,
): BaggageEntryMetadata {
  if (typeof str !== "string") {
    str = "";
  }

  return {
    __TYPE__: baggageEntryMetadataSymbol,
    toString() {
      return str;
    },
  };
}

class BaggageImpl implements Baggage {
  #entries: Map<string, BaggageEntry>;

  constructor(entries?: Map<string, BaggageEntry>) {
    this.#entries = new SafeMap();
    // The `SafeMap` constructor that takes an iterable doesn't work for non Array iterables correctly.
    if (entries) {
      for (const { 0: key, 1: entry } of new SafeMapIterator(entries)) {
        this.#entries.set(key, ObjectAssign({}, entry));
      }
    }
  }

  getEntry(key: string): BaggageEntry | undefined {
    const entry = this.#entries.get(key);
    if (!entry) {
      return undefined;
    }

    return ObjectAssign({}, entry);
  }

  getAllEntries(): [string, BaggageEntry][] {
    return ArrayPrototypeMap(
      ArrayFrom(MapPrototypeEntries(this.#entries)),
      (entry) => [entry[0], entry[1]],
    );
  }

  setEntry(key: string, entry: BaggageEntry): BaggageImpl {
    const newBaggage = new BaggageImpl(this.#entries);
    newBaggage.#entries.set(key, entry);
    return newBaggage;
  }

  removeEntry(key: string): BaggageImpl {
    const newBaggage = new BaggageImpl(this.#entries);
    newBaggage.#entries.delete(key);
    return newBaggage;
  }

  removeEntries(...keys: string[]): BaggageImpl {
    const newBaggage = new BaggageImpl(this.#entries);
    for (const key of new SafeArrayIterator(keys)) {
      newBaggage.#entries.delete(key);
    }
    return newBaggage;
  }

  clear(): BaggageImpl {
    return new BaggageImpl();
  }
}

const BAGGAGE_KEY = SymbolFor("OpenTelemetry Baggage Key");

class W3CBaggagePropagator implements TextMapPropagator {
  inject(context: Context, carrier: unknown, setter: TextMapSetter): void {
    const baggage = context.getValue(BAGGAGE_KEY) as
      | Baggage
      | undefined;
    if (!baggage || isTracingSuppressed(context)) return;
    // Percent-encoding, per-pair / total length limits and the maximum number
    // of members are all enforced by the Rust op.
    const entries = ArrayPrototypeMap(
      baggage.getAllEntries(),
      (baggageEntry) => ({
        key: baggageEntry[0],
        value: baggageEntry[1].value,
        metadata: baggageEntry[1].metadata !== undefined
          // deno-lint-ignore prefer-primordials
          ? baggageEntry[1].metadata.toString()
          : undefined,
      }),
    );
    const headerValue = op_otel_baggage_serialize(entries);
    if (headerValue.length > 0) {
      setter.set(carrier, BAGGAGE_HEADER, headerValue);
    }
  }

  extract(context: Context, carrier: unknown, getter: TextMapGetter): Context {
    const headerValue = getter.get(carrier, BAGGAGE_HEADER);
    const baggageString = ArrayIsArray(headerValue)
      ? ArrayPrototypeJoin(headerValue, BAGGAGE_ITEMS_SEPARATOR)
      : headerValue;
    if (!baggageString) return context;
    // Parsing, percent-decoding and de-duplication is performed in Rust.
    const entries = op_otel_baggage_parse(baggageString);
    if (entries.length === 0) return context;
    const map = new SafeMap<string, BaggageEntry>();
    for (let i = 0; i < entries.length; i++) {
      const entry = entries[i];
      const baggageEntry: BaggageEntry = { value: entry.value };
      if (entry.metadata != null) {
        baggageEntry.metadata = baggageEntryMetadataFromString(entry.metadata);
      }
      map.set(entry.key, baggageEntry);
    }
    return context.setValue(BAGGAGE_KEY, new BaggageImpl(map));
  }

  fields(): string[] {
    return [BAGGAGE_HEADER];
  }
}

class CompositePropagator implements TextMapPropagator {
  #propagators: TextMapPropagator[];
  #fields: string[];

  constructor(propagators: TextMapPropagator[]) {
    this.#propagators = propagators;
    this.#fields = ArrayFrom(
      new SafeSet(
        ArrayPrototypeReduce(
          ArrayPrototypeMap(
            this.#propagators,
            (p) => p.fields(),
          ),
          (x, y) => ArrayPrototypeConcat(x, y),
          [],
        ),
      ),
    );
  }

  inject(context: Context, carrier: unknown, setter: TextMapSetter): void {
    for (const propagator of new SafeArrayIterator(this.#propagators)) {
      try {
        propagator.inject(context, carrier, setter);
      } catch (err) {
        // deno-lint-ignore no-console
        console.warn(
          `Failed to inject with ${propagator.constructor.name}.`,
          err,
        );
      }
    }
  }

  extract(context: Context, carrier: unknown, getter: TextMapGetter): Context {
    return ArrayPrototypeReduce(this.#propagators, (ctx, propagator) => {
      try {
        return propagator.extract(ctx, carrier, getter);
      } catch (err) {
        // deno-lint-ignore no-console
        console.warn(
          `Failed to extract with ${propagator.constructor.name}.`,
          err,
        );
      }
      return ctx;
    }, context);
  }

  fields(): string[] {
    return ArrayPrototypeSlice(this.#fields);
  }
}

let builtinTracerCache: Tracer;

function builtinTracer(): Tracer {
  if (!builtinTracerCache) {
    builtinTracerCache = new Tracer(OtelTracer.builtin());
  }
  return builtinTracerCache;
}

function enableIsolateMetrics() {
  op_otel_enable_isolate_metrics();
  ISOLATE_METRICS = true;
  startObserving();
}

// We specify a very high version number, to allow any `@opentelemetry/api`
// version to load this module. This does cause @opentelemetry/api to not be
// able to register anything itself with the global registration methods.
const OTEL_API_COMPAT_VERSION = "1.999.999";

function bootstrap(
  config: [
    0 | 1,
    0 | 1,
    (typeof otelConsoleConfig)[keyof typeof otelConsoleConfig],
    ...Array<(typeof otelPropagators)[keyof typeof otelPropagators]>,
  ],
): void {
  const {
    0: tracingEnabled,
    1: metricsEnabled,
    2: consoleConfig,
    ...propagators
  } = config;

  TRACING_ENABLED = tracingEnabled === 1;
  METRICS_ENABLED = metricsEnabled === 1;

  PROPAGATORS = ArrayPrototypeMap(
    ArrayPrototypeFilter(
      ObjectValues(propagators),
      (propagator) => propagator !== otelPropagators.none,
    ),
    (propagator) => {
      switch (propagator) {
        case otelPropagators.traceContext:
          return new W3CTraceContextPropagator();
        case otelPropagators.baggage:
          return new W3CBaggagePropagator();
      }
    },
  );

  switch (consoleConfig) {
    case otelConsoleConfig.capture: {
      const otelConsole = new Console(otelLog);
      wrapOtelConsoleMethods(otelConsole);
      core.wrapConsole(globalThis.console, otelConsole);
      break;
    }
    case otelConsoleConfig.replace: {
      const otelConsole = new Console(otelLog);
      wrapOtelConsoleMethods(otelConsole);
      ObjectDefineProperty(
        globalThis,
        "console",
        core.propNonEnumerable(otelConsole),
      );
      break;
    }
    default:
      break;
  }

  if (TRACING_ENABLED || METRICS_ENABLED || PROPAGATORS.length > 0) {
    const otel = globalThis[SymbolFor("opentelemetry.js.api.1")] ??= {
      version: OTEL_API_COMPAT_VERSION,
    };
    if (TRACING_ENABLED) {
      otel.trace = TracerProvider;
      otel.context = ContextManager;
    }
    if (METRICS_ENABLED) {
      otel.metrics = MeterProvider;
      enableIsolateMetrics();
    }
    if (PROPAGATORS.length > 0) {
      otel.propagation = new CompositePropagator(PROPAGATORS);
    }
  }
}

internals.__telemetry = {
  builtinTracer,
  ContextManager,
  enterSpan,
  get PROPAGATORS() {
    return PROPAGATORS;
  },
  restoreSnapshot,
  get TRACING_ENABLED() {
    return TRACING_ENABLED;
  },
};

const telemetry = {
  tracerProvider: TracerProvider,
  contextManager: ContextManager,
  meterProvider: MeterProvider,
};

// Mutable state container: consumers destructure a reference to this
// object, so property access at call-time always reflects the latest
// values set by bootstrap().
const otelState = {
  TRACING_ENABLED: false,
  METRICS_ENABLED: false,
  PROPAGATORS: [] as TextMapPropagator[],
  getOtelSpan: undefined as
    | ((span: object) => OtelSpan | null | undefined)
    | undefined,
};

// Keep module-level variables in sync with otelState for internal use
// (existing code references the bare names).
const _origBootstrap = bootstrap;
function wrappedBootstrap(config: Parameters<typeof bootstrap>[0]) {
  _origBootstrap(config);
  otelState.TRACING_ENABLED = TRACING_ENABLED;
  otelState.METRICS_ENABLED = METRICS_ENABLED;
  otelState.PROPAGATORS = PROPAGATORS;
  otelState.getOtelSpan = getOtelSpan;
}

return {
  otelState,
  enterSpan,
  currentSnapshot,
  restoreSnapshot,
  SPAN_KEY,
  Span,
  ContextManager,
  baggageEntryMetadataFromString,
  W3CBaggagePropagator,
  CompositePropagator,
  builtinTracer,
  bootstrap: wrappedBootstrap,
  telemetry,
};
})();
