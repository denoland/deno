// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeFilter,
  ArrayPrototypePush,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ReflectHas,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { structuredClone } from "./02_structured_clone.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { EventTarget } from "./02_event.js";
import { opNow } from "./02_timers.js";
import { DOMException } from "./01_dom_exception.js";

const illegalConstructorKey = Symbol("illegalConstructorKey");
let performanceEntries = [];
let timeOrigin;

webidl.converters["PerformanceMarkOptions"] = webidl
  .createDictionaryConverter(
    "PerformanceMarkOptions",
    [
      {
        key: "detail",
        converter: webidl.converters.any,
      },
      {
        key: "startTime",
        converter: webidl.converters.DOMHighResTimeStamp,
      },
    ],
  );

webidl.converters["DOMString or DOMHighResTimeStamp"] = (
  V,
  prefix,
  context,
  opts,
) => {
  if (webidl.type(V) === "Number" && V !== null) {
    return webidl.converters.DOMHighResTimeStamp(V, prefix, context, opts);
  }
  return webidl.converters.DOMString(V, prefix, context, opts);
};

webidl.converters["PerformanceMeasureOptions"] = webidl
  .createDictionaryConverter(
    "PerformanceMeasureOptions",
    [
      {
        key: "detail",
        converter: webidl.converters.any,
      },
      {
        key: "start",
        converter: webidl.converters["DOMString or DOMHighResTimeStamp"],
      },
      {
        key: "duration",
        converter: webidl.converters.DOMHighResTimeStamp,
      },
      {
        key: "end",
        converter: webidl.converters["DOMString or DOMHighResTimeStamp"],
      },
    ],
  );

webidl.converters["DOMString or PerformanceMeasureOptions"] = (
  V,
  prefix,
  context,
  opts,
) => {
  if (webidl.type(V) === "Object" && V !== null) {
    return webidl.converters["PerformanceMeasureOptions"](
      V,
      prefix,
      context,
      opts,
    );
  }
  return webidl.converters.DOMString(V, prefix, context, opts);
};

function setTimeOrigin(origin) {
  timeOrigin = origin;
}

function findMostRecent(
  name,
  type,
) {
  for (let i = performanceEntries.length - 1; i >= 0; --i) {
    const entry = performanceEntries[i];
    if (entry.name === name && entry.entryType === type) {
      return entry;
    }
  }
}

function convertMarkToTimestamp(mark) {
  if (typeof mark === "string") {
    const entry = findMostRecent(mark, "mark");
    if (!entry) {
      throw new DOMException(
        `Cannot find mark: "${mark}".`,
        "SyntaxError",
      );
    }
    return entry.startTime;
  }
  if (mark < 0) {
    throw new TypeError("Mark cannot be negative.");
  }
  return mark;
}

function filterByNameType(
  name,
  type,
) {
  return ArrayPrototypeFilter(
    performanceEntries,
    (entry) =>
      (name ? entry.name === name : true) &&
      (type ? entry.entryType === type : true),
  );
}

const now = opNow;

const _name = Symbol("[[name]]");
const _entryType = Symbol("[[entryType]]");
const _startTime = Symbol("[[startTime]]");
const _duration = Symbol("[[duration]]");
class PerformanceEntry {
  [_name] = "";
  [_entryType] = "";
  [_startTime] = 0;
  [_duration] = 0;

  get name() {
    webidl.assertBranded(this, PerformanceEntryPrototype);
    return this[_name];
  }

  get entryType() {
    webidl.assertBranded(this, PerformanceEntryPrototype);
    return this[_entryType];
  }

  get startTime() {
    webidl.assertBranded(this, PerformanceEntryPrototype);
    return this[_startTime];
  }

  get duration() {
    webidl.assertBranded(this, PerformanceEntryPrototype);
    return this[_duration];
  }

  constructor(
    name = null,
    entryType = null,
    startTime = null,
    duration = null,
    key = undefined,
  ) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this[webidl.brand] = webidl.brand;

    this[_name] = name;
    this[_entryType] = entryType;
    this[_startTime] = startTime;
    this[_duration] = duration;
  }

  toJSON() {
    webidl.assertBranded(this, PerformanceEntryPrototype);
    return {
      name: this[_name],
      entryType: this[_entryType],
      startTime: this[_startTime],
      duration: this[_duration],
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          PerformanceEntryPrototype,
          this,
        ),
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
        ],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(PerformanceEntry);
const PerformanceEntryPrototype = PerformanceEntry.prototype;

const _detail = Symbol("[[detail]]");
class PerformanceMark extends PerformanceEntry {
  [_detail] = null;

  get detail() {
    webidl.assertBranded(this, PerformanceMarkPrototype);
    return this[_detail];
  }

  get entryType() {
    webidl.assertBranded(this, PerformanceMarkPrototype);
    return "mark";
  }

  constructor(
    name,
    options = { __proto__: null },
  ) {
    const prefix = "Failed to construct 'PerformanceMark'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters.DOMString(name, prefix, "Argument 1");

    options = webidl.converters.PerformanceMarkOptions(
      options,
      prefix,
      "Argument 2",
    );

    const { detail = null, startTime = now() } = options;

    super(name, "mark", startTime, 0, illegalConstructorKey);
    this[webidl.brand] = webidl.brand;
    if (startTime < 0) {
      throw new TypeError("startTime cannot be negative");
    }
    this[_detail] = structuredClone(detail);
  }

  toJSON() {
    webidl.assertBranded(this, PerformanceMarkPrototype);
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      detail: this.detail,
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(PerformanceMarkPrototype, this),
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
          "detail",
        ],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(PerformanceMark);
const PerformanceMarkPrototype = PerformanceMark.prototype;
class PerformanceMeasure extends PerformanceEntry {
  [_detail] = null;

  get detail() {
    webidl.assertBranded(this, PerformanceMeasurePrototype);
    return this[_detail];
  }

  get entryType() {
    webidl.assertBranded(this, PerformanceMeasurePrototype);
    return "measure";
  }

  constructor(
    name = null,
    startTime = null,
    duration = null,
    detail = null,
    key = undefined,
  ) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }

    super(name, "measure", startTime, duration, key);
    this[webidl.brand] = webidl.brand;
    this[_detail] = structuredClone(detail);
  }

  toJSON() {
    webidl.assertBranded(this, PerformanceMeasurePrototype);
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      detail: this.detail,
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          PerformanceMeasurePrototype,
          this,
        ),
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
          "detail",
        ],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(PerformanceMeasure);
const PerformanceMeasurePrototype = PerformanceMeasure.prototype;
class Performance extends EventTarget {
  constructor(key = null) {
    if (key != illegalConstructorKey) {
      webidl.illegalConstructor();
    }

    super();
    this[webidl.brand] = webidl.brand;
  }

  get timeOrigin() {
    webidl.assertBranded(this, PerformancePrototype);
    return timeOrigin;
  }

  clearMarks(markName = undefined) {
    webidl.assertBranded(this, PerformancePrototype);
    if (markName !== undefined) {
      markName = webidl.converters.DOMString(
        markName,
        "Failed to execute 'clearMarks' on 'Performance'",
        "Argument 1",
      );

      performanceEntries = ArrayPrototypeFilter(
        performanceEntries,
        (entry) => !(entry.name === markName && entry.entryType === "mark"),
      );
    } else {
      performanceEntries = ArrayPrototypeFilter(
        performanceEntries,
        (entry) => entry.entryType !== "mark",
      );
    }
  }

  clearMeasures(measureName = undefined) {
    webidl.assertBranded(this, PerformancePrototype);
    if (measureName !== undefined) {
      measureName = webidl.converters.DOMString(
        measureName,
        "Failed to execute 'clearMeasures' on 'Performance'",
        "Argument 1",
      );

      performanceEntries = ArrayPrototypeFilter(
        performanceEntries,
        (entry) =>
          !(entry.name === measureName && entry.entryType === "measure"),
      );
    } else {
      performanceEntries = ArrayPrototypeFilter(
        performanceEntries,
        (entry) => entry.entryType !== "measure",
      );
    }
  }

  getEntries() {
    webidl.assertBranded(this, PerformancePrototype);
    return filterByNameType();
  }

  getEntriesByName(
    name,
    type = undefined,
  ) {
    webidl.assertBranded(this, PerformancePrototype);
    const prefix = "Failed to execute 'getEntriesByName' on 'Performance'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters.DOMString(name, prefix, "Argument 1");

    if (type !== undefined) {
      type = webidl.converters.DOMString(type, prefix, "Argument 2");
    }

    return filterByNameType(name, type);
  }

  getEntriesByType(type) {
    webidl.assertBranded(this, PerformancePrototype);
    const prefix = "Failed to execute 'getEntriesByName' on 'Performance'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    type = webidl.converters.DOMString(type, prefix, "Argument 1");

    return filterByNameType(undefined, type);
  }

  mark(
    markName,
    markOptions = { __proto__: null },
  ) {
    webidl.assertBranded(this, PerformancePrototype);
    const prefix = "Failed to execute 'mark' on 'Performance'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    markName = webidl.converters.DOMString(markName, prefix, "Argument 1");

    markOptions = webidl.converters.PerformanceMarkOptions(
      markOptions,
      prefix,
      "Argument 2",
    );

    // 3.1.1.1 If the global object is a Window object and markName uses the
    // same name as a read only attribute in the PerformanceTiming interface,
    // throw a SyntaxError. - not implemented
    const entry = new PerformanceMark(markName, markOptions);
    // 3.1.1.7 Queue entry - not implemented
    ArrayPrototypePush(performanceEntries, entry);
    return entry;
  }

  measure(
    measureName,
    startOrMeasureOptions = { __proto__: null },
    endMark = undefined,
  ) {
    webidl.assertBranded(this, PerformancePrototype);
    const prefix = "Failed to execute 'measure' on 'Performance'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    measureName = webidl.converters.DOMString(
      measureName,
      prefix,
      "Argument 1",
    );

    startOrMeasureOptions = webidl.converters
      ["DOMString or PerformanceMeasureOptions"](
        startOrMeasureOptions,
        prefix,
        "Argument 2",
      );

    if (endMark !== undefined) {
      endMark = webidl.converters.DOMString(endMark, prefix, "Argument 3");
    }

    if (
      startOrMeasureOptions && typeof startOrMeasureOptions === "object" &&
      ObjectKeys(startOrMeasureOptions).length > 0
    ) {
      if (endMark) {
        throw new TypeError("Options cannot be passed with endMark.");
      }
      if (
        !ReflectHas(startOrMeasureOptions, "start") &&
        !ReflectHas(startOrMeasureOptions, "end")
      ) {
        throw new TypeError(
          "A start or end mark must be supplied in options.",
        );
      }
      if (
        ReflectHas(startOrMeasureOptions, "start") &&
        ReflectHas(startOrMeasureOptions, "duration") &&
        ReflectHas(startOrMeasureOptions, "end")
      ) {
        throw new TypeError(
          "Cannot specify start, end, and duration together in options.",
        );
      }
    }
    let endTime;
    if (endMark) {
      endTime = convertMarkToTimestamp(endMark);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      ReflectHas(startOrMeasureOptions, "end")
    ) {
      endTime = convertMarkToTimestamp(startOrMeasureOptions.end);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      ReflectHas(startOrMeasureOptions, "start") &&
      ReflectHas(startOrMeasureOptions, "duration")
    ) {
      const start = convertMarkToTimestamp(startOrMeasureOptions.start);
      const duration = convertMarkToTimestamp(startOrMeasureOptions.duration);
      endTime = start + duration;
    } else {
      endTime = now();
    }
    let startTime;
    if (
      typeof startOrMeasureOptions === "object" &&
      ReflectHas(startOrMeasureOptions, "start")
    ) {
      startTime = convertMarkToTimestamp(startOrMeasureOptions.start);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      ReflectHas(startOrMeasureOptions, "end") &&
      ReflectHas(startOrMeasureOptions, "duration")
    ) {
      const end = convertMarkToTimestamp(startOrMeasureOptions.end);
      const duration = convertMarkToTimestamp(startOrMeasureOptions.duration);
      startTime = end - duration;
    } else if (typeof startOrMeasureOptions === "string") {
      startTime = convertMarkToTimestamp(startOrMeasureOptions);
    } else {
      startTime = 0;
    }
    const entry = new PerformanceMeasure(
      measureName,
      startTime,
      endTime - startTime,
      typeof startOrMeasureOptions === "object"
        ? startOrMeasureOptions.detail ?? null
        : null,
      illegalConstructorKey,
    );
    ArrayPrototypePush(performanceEntries, entry);
    return entry;
  }

  now() {
    webidl.assertBranded(this, PerformancePrototype);
    return now();
  }

  toJSON() {
    webidl.assertBranded(this, PerformancePrototype);
    return {
      timeOrigin: this.timeOrigin,
    };
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(PerformancePrototype, this),
        keys: ["timeOrigin"],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(Performance);
const PerformancePrototype = Performance.prototype;

webidl.converters["Performance"] = webidl.createInterfaceConverter(
  "Performance",
  PerformancePrototype,
);

const performance = new Performance(illegalConstructorKey);

export {
  Performance,
  performance,
  PerformanceEntry,
  PerformanceMark,
  PerformanceMeasure,
  setTimeOrigin,
};
