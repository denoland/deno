// Copyright 2018-2026 the Deno authors. MIT license.

import {
  metrics,
  type ObservableResult,
  trace,
} from "npm:@opentelemetry/api@1.9.0";

function assertGetterError(callback: () => void, expected: Error) {
  let actual: unknown;
  try {
    callback();
  } catch (error) {
    actual = error;
  }
  if (actual !== expected) {
    throw new Error(`expected ${expected}, got ${String(actual)}`);
  }
}

const tracer = trace.getTracer("reentrant-tracer");
const span = tracer.startSpan("reentrant attribute span");

let spanGetterCount = 0;
const spanArray = [];
Object.defineProperty(spanArray, 0, {
  get() {
    spanGetterCount++;
    Deno.cwd();
    span.end();
    return "trigger";
  },
});

span.setAttribute("reentrant", spanArray);
if (spanGetterCount === 0) {
  throw new Error("span getter was not triggered");
}

let endedSpanGetterCount = 0;
const endedSpanArray: string[] = [];
Object.defineProperty(endedSpanArray, 0, {
  get() {
    endedSpanGetterCount++;
    throw new Error("ended span getter should not run");
  },
});
span.setAttribute("ignored", endedSpanArray);
if (endedSpanGetterCount !== 0) {
  throw new Error("ended span getter was triggered");
}

const throwingSpan = tracer.startSpan("throwing attribute span");
const throwingSpanArray: string[] = [];
const throwingSpanError = new Error("span getter boom");
Object.defineProperty(throwingSpanArray, 0, {
  get() {
    throw throwingSpanError;
  },
});
assertGetterError(
  () => throwingSpan.setAttribute("throwing", throwingSpanArray),
  throwingSpanError,
);
throwingSpan.end();

const fullSpan = tracer.startSpan("full attribute span");
fullSpan.setAttribute("first", "value");
let fullSpanGetterCount = 0;
const fullSpanArray: string[] = [];
Object.defineProperty(fullSpanArray, 0, {
  get() {
    fullSpanGetterCount++;
    throw new Error("full span getter should not run");
  },
});
fullSpan.setAttribute("ignored", fullSpanArray);
if (fullSpanGetterCount !== 0) {
  throw new Error("full span getter was triggered");
}
fullSpan.end();

const targetSpan = tracer.startSpan("stable attribute targets span");
let eventGetterCount = 0;
const eventArray: string[] = [];
Object.defineProperty(eventArray, 0, {
  get() {
    eventGetterCount++;
    if (eventGetterCount === 1) {
      targetSpan.addEvent("nested event");
    }
    return "event-trigger";
  },
});
targetSpan.addEvent("outer event", { reentrant: eventArray });
if (eventGetterCount === 0) {
  throw new Error("event getter was not triggered");
}

const nestedLink = {
  context: {
    traceId: "00000000000000000000000000000011",
    spanId: "0000000000000011",
    traceFlags: 1,
  },
};
let linkGetterCount = 0;
const linkArray: string[] = [];
Object.defineProperty(linkArray, 0, {
  get() {
    linkGetterCount++;
    if (linkGetterCount === 1) {
      targetSpan.addLink(nestedLink);
    }
    return "link-trigger";
  },
});
targetSpan.addLink({
  context: {
    traceId: "00000000000000000000000000000010",
    spanId: "0000000000000010",
    traceFlags: 1,
  },
  attributes: { reentrant: linkArray },
});
if (linkGetterCount === 0) {
  throw new Error("link getter was not triggered");
}
targetSpan.end();

const meter = metrics.getMeter("reentrant-meter");
const counter = meter.createCounter("reentrant.counter");
const nestedCounter = meter.createCounter("reentrant.nested");

let metricGetterCount = 0;
const metricArray: string[] = [];
Object.defineProperty(metricArray, 0, {
  get() {
    metricGetterCount++;
    Deno.cwd();
    nestedCounter.add(1);
    return "metric-trigger";
  },
});

counter.add(1, {
  first: "one",
  second: "two",
  third: "three",
  reentrant: metricArray,
  fifth: "five",
  sixth: "six",
  seventh: "seven",
});
if (metricGetterCount === 0) {
  throw new Error("metric getter was not triggered");
}

const throwingMetricArray: string[] = [];
const throwingMetricError = new Error("metric getter boom");
Object.defineProperty(throwingMetricArray, 0, {
  get() {
    throw throwingMetricError;
  },
});
assertGetterError(
  () =>
    counter.add(2, {
      first: "one",
      second: "two",
      third: "three",
      throwing: throwingMetricArray,
      fifth: "five",
      sixth: "six",
      seventh: "seven",
    }),
  throwingMetricError,
);
counter.add(3);

const observableDone = Promise.withResolvers<void>();
const observable = meter.createObservableCounter("reentrant.observable");
let observableGetterCount = 0;
const observableCallback = (result: ObservableResult) => {
  try {
    const observableArray: string[] = [];
    Object.defineProperty(observableArray, 0, {
      get() {
        observableGetterCount++;
        Deno.cwd();
        nestedCounter.add(1);
        return "observable-trigger";
      },
    });
    result.observe(1, {
      first: "one",
      second: "two",
      third: "three",
      reentrant: observableArray,
    });

    const throwingObservableArray: string[] = [];
    const throwingObservableError = new Error("observable getter boom");
    Object.defineProperty(throwingObservableArray, 0, {
      get() {
        throw throwingObservableError;
      },
    });
    assertGetterError(
      () =>
        result.observe(2, {
          first: "one",
          second: "two",
          third: "three",
          throwing: throwingObservableArray,
        }),
      throwingObservableError,
    );
    result.observe(3);
    observableDone.resolve();
  } catch (error) {
    observableDone.reject(error);
  }
};
observable.addCallback(observableCallback);

const timer = setTimeout(() => {}, 100_000);
try {
  await observableDone.promise;
} finally {
  clearTimeout(timer);
  observable.removeCallback(observableCallback);
}
if (observableGetterCount === 0) {
  throw new Error("observable getter was not triggered");
}
