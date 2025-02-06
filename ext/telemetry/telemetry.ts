// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
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
  op_otel_span_add_link,
  op_otel_span_attribute1,
  op_otel_span_attribute2,
  op_otel_span_attribute3,
  op_otel_span_update_name,
  OtelMeter,
  OtelSpan,
  OtelTracer,
} from "ext:core/ops";
import { Console } from "ext:deno_console/01_console.js";

const {
  ArrayIsArray,
  ArrayPrototypePush,
  DatePrototype,
  DatePrototypeGetTime,
  Error,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SafeIterator,
  SafeMap,
  SafePromiseAll,
  SafeSet,
  SafeWeakSet,
  SymbolFor,
  TypeError,
} = primordials;
const { AsyncVariable, setAsyncContext } = core;

export let TRACING_ENABLED = false;
export let METRICS_ENABLED = false;

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

export function enterSpan(span: Span): Context | undefined {
  if (!span.isRecording()) return undefined;
  const context = (CURRENT.get() || ROOT_CONTEXT).setValue(SPAN_KEY, span);
  return CURRENT.enter(context);
}

export function restoreContext(context: Context): void {
  setAsyncContext(context);
}

function isDate(value: unknown): value is Date {
  return ObjectPrototypeIsPrototypeOf(value, DatePrototype);
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
  dropEvent(): void;
  end(endTime: number): void;
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
      context = undefined;
    } else {
      context = context ?? CURRENT.get();
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

    let startTime = options?.startTime;
    if (startTime && ArrayIsArray(startTime)) {
      startTime = hrToMs(startTime);
    } else if (startTime && isDate(startTime)) {
      startTime = DatePrototypeGetTime(startTime);
    }

    const parentSpan = context?.getValue(SPAN_KEY) as
      | Span
      | { spanContext(): SpanContext }
      | undefined;
    const attributesCount = options?.attributes
      ? ObjectKeys(options.attributes).length
      : 0;
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
    // deno-lint-ignore prefer-primordials
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
    _name: string,
    _attributesOrStartTime?: Attributes | TimeInput,
    _startTime?: TimeInput,
  ): Span {
    this.#otelSpan?.dropEvent();
    return this;
  }

  addLink(link: Link): Span {
    const droppedAttributeCount = (link.droppedAttributesCount ?? 0) +
      (link.attributes ? ObjectKeys(link.attributes).length : 0);
    const valid = op_otel_span_add_link(
      this.#otelSpan,
      link.context.traceId,
      link.context.spanId,
      link.context.traceFlags,
      link.context.isRemote ?? false,
      droppedAttributeCount,
    );
    if (!valid) return this;
    return this;
  }

  addLinks(links: Link[]): Span {
    for (let i = 0; i < links.length; i++) {
      this.addLink(links[i]);
    }
    return this;
  }

  end(endTime?: TimeInput): void {
    if (endTime && ArrayIsArray(endTime)) {
      endTime = hrToMs(endTime);
    } else if (endTime && isDate(endTime)) {
      endTime = DatePrototypeGetTime(endTime);
    }
    this.#otelSpan?.end(endTime || NaN);
  }

  isRecording(): boolean {
    return this.#otelSpan !== undefined;
  }

  // deno-lint-ignore no-explicit-any
  recordException(_exception: any, _time?: TimeInput): Span {
    this.#otelSpan?.dropEvent();
    return this;
  }

  setAttribute(key: string, value: AttributeValue): Span {
    if (!this.#otelSpan) return this;
    op_otel_span_attribute1(this.#otelSpan, key, value);
    return this;
  }

  setAttributes(attributes: Attributes): Span {
    if (!this.#otelSpan) return this;
    const attributeKvs = ObjectEntries(attributes);
    let i = 0;
    while (i < attributeKvs.length) {
      if (i + 2 < attributeKvs.length) {
        op_otel_span_attribute3(
          this.#otelSpan,
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
          this.#otelSpan,
          attributeKvs[i][0],
          attributeKvs[i][1],
          attributeKvs[i + 1][0],
          attributeKvs[i + 1][1],
        );
        i += 2;
      } else {
        op_otel_span_attribute1(
          this.#otelSpan,
          attributeKvs[i][0],
          attributeKvs[i][1],
        );
        i += 1;
      }
    }
    return this;
  }

  setStatus(status: SpanStatus): Span {
    this.#otelSpan?.setStatus(status.code, status.message ?? "");
    return this;
  }

  updateName(name: string): Span {
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
      // deno-lint-ignore prefer-primordials
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
      // deno-lint-ignore prefer-primordials
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
      // deno-lint-ignore prefer-primordials
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
      // deno-lint-ignore prefer-primordials
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
      // deno-lint-ignore prefer-primordials
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
      // deno-lint-ignore prefer-primordials
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

function otelLog(message: string, level: number) {
  const currentSpan = CURRENT.get()?.getValue(SPAN_KEY);
  const otelSpan = currentSpan !== undefined
    ? getOtelSpan(currentSpan)
    : undefined;
  if (otelSpan || currentSpan === undefined) {
    op_otel_log(message, level, otelSpan);
  } else {
    const spanContext = currentSpan.spanContext();
    op_otel_log_foreign(
      message,
      level,
      spanContext.traceId,
      spanContext.spanId,
      spanContext.traceFlags,
    );
  }
}

let builtinTracerCache: Tracer;

export function builtinTracer(): Tracer {
  if (!builtinTracerCache) {
    builtinTracerCache = new Tracer(OtelTracer.builtin());
  }
  return builtinTracerCache;
}

// We specify a very high version number, to allow any `@opentelemetry/api`
// version to load this module. This does cause @opentelemetry/api to not be
// able to register anything itself with the global registration methods.
const OTEL_API_COMPAT_VERSION = "1.999.999";

export function bootstrap(
  config: [
    0 | 1,
    0 | 1,
    (typeof otelConsoleConfig)[keyof typeof otelConsoleConfig],
    0 | 1,
  ],
): void {
  const { 0: tracingEnabled, 1: metricsEnabled, 2: consoleConfig } = config;

  TRACING_ENABLED = tracingEnabled === 1;
  METRICS_ENABLED = metricsEnabled === 1;

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

  if (TRACING_ENABLED || METRICS_ENABLED) {
    const otel = globalThis[SymbolFor("opentelemetry.js.api.1")] ??= {
      version: OTEL_API_COMPAT_VERSION,
    };
    if (TRACING_ENABLED) {
      otel.trace = TracerProvider;
      otel.context = ContextManager;
    }
    if (METRICS_ENABLED) {
      otel.metrics = MeterProvider;
    }
  }
}

export const telemetry = {
  tracerProvider: TracerProvider,
  contextManager: ContextManager,
  meterProvider: MeterProvider,
};
