// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    ArrayPrototypeFilter,
    ArrayPrototypeFind,
    ArrayPrototypePush,
    ArrayPrototypeReverse,
    ArrayPrototypeSlice,
    ObjectKeys,
    Symbol,
    SymbolFor,
    SymbolToStringTag,
    TypeError,
  } = window.__bootstrap.primordials;

  const { webidl, structuredClone } = window.__bootstrap;
  const consoleInternal = window.__bootstrap.console;
  const { opNow } = window.__bootstrap.timers;
  const { DOMException } = window.__bootstrap.domException;

  const illegalConstructorKey = Symbol("illegalConstructorKey");
  const customInspect = SymbolFor("Deno.customInspect");
  let performanceEntries = [];

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

  webidl.converters["DOMString or DOMHighResTimeStamp"] = (V, opts) => {
    if (webidl.type(V) === "Number" && V !== null) {
      return webidl.converters.DOMHighResTimeStamp(V, opts);
    }
    return webidl.converters.DOMString(V, opts);
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

  webidl.converters["DOMString or PerformanceMeasureOptions"] = (V, opts) => {
    if (webidl.type(V) === "Object" && V !== null) {
      return webidl.converters["PerformanceMeasureOptions"](V, opts);
    }
    return webidl.converters.DOMString(V, opts);
  };

  function findMostRecent(
    name,
    type,
  ) {
    return ArrayPrototypeFind(
      ArrayPrototypeReverse(ArrayPrototypeSlice(performanceEntries)),
      (entry) => entry.name === name && entry.entryType === type,
    );
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
      webidl.assertBranded(this, PerformanceEntry);
      return this[_name];
    }

    get entryType() {
      webidl.assertBranded(this, PerformanceEntry);
      return this[_entryType];
    }

    get startTime() {
      webidl.assertBranded(this, PerformanceEntry);
      return this[_startTime];
    }

    get duration() {
      webidl.assertBranded(this, PerformanceEntry);
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
      webidl.assertBranded(this, PerformanceEntry);
      return {
        name: this[_name],
        entryType: this[_entryType],
        startTime: this[_startTime],
        duration: this[_duration],
      };
    }

    [customInspect](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof PerformanceEntry,
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
        ],
      }));
    }
  }
  webidl.configurePrototype(PerformanceEntry);

  const _detail = Symbol("[[detail]]");
  class PerformanceMark extends PerformanceEntry {
    [SymbolToStringTag] = "PerformanceMark";

    [_detail] = null;

    get detail() {
      webidl.assertBranded(this, PerformanceMark);
      return this[_detail];
    }

    get entryType() {
      webidl.assertBranded(this, PerformanceMark);
      return "mark";
    }

    constructor(
      name,
      options = {},
    ) {
      const prefix = "Failed to construct 'PerformanceMark'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });

      options = webidl.converters.PerformanceMarkOptions(options, {
        prefix,
        context: "Argument 2",
      });

      const { detail = null, startTime = now() } = options;

      super(name, "mark", startTime, 0, illegalConstructorKey);
      this[webidl.brand] = webidl.brand;
      if (startTime < 0) {
        throw new TypeError("startTime cannot be negative");
      }
      this[_detail] = structuredClone(detail);
    }

    toJSON() {
      webidl.assertBranded(this, PerformanceMark);
      return {
        name: this.name,
        entryType: this.entryType,
        startTime: this.startTime,
        duration: this.duration,
        detail: this.detail,
      };
    }

    [customInspect](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof PerformanceMark,
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
          "detail",
        ],
      }));
    }
  }
  webidl.configurePrototype(PerformanceMark);

  class PerformanceMeasure extends PerformanceEntry {
    [SymbolToStringTag] = "PerformanceMeasure";

    [_detail] = null;

    get detail() {
      webidl.assertBranded(this, PerformanceMeasure);
      return this[_detail];
    }

    get entryType() {
      webidl.assertBranded(this, PerformanceMeasure);
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
      webidl.assertBranded(this, PerformanceMeasure);
      return {
        name: this.name,
        entryType: this.entryType,
        startTime: this.startTime,
        duration: this.duration,
        detail: this.detail,
      };
    }

    [customInspect](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof PerformanceMeasure,
        keys: [
          "name",
          "entryType",
          "startTime",
          "duration",
          "detail",
        ],
      }));
    }
  }
  webidl.configurePrototype(PerformanceMeasure);

  class Performance {
    constructor() {
      webidl.illegalConstructor();
    }

    clearMarks(markName = undefined) {
      webidl.assertBranded(this, Performance);
      if (markName !== undefined) {
        markName = webidl.converters.DOMString(markName, {
          prefix: "Failed to execute 'clearMarks' on 'Performance'",
          context: "Argument 1",
        });

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
      webidl.assertBranded(this, Performance);
      if (measureName !== undefined) {
        measureName = webidl.converters.DOMString(measureName, {
          prefix: "Failed to execute 'clearMeasures' on 'Performance'",
          context: "Argument 1",
        });

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
      webidl.assertBranded(this, Performance);
      return filterByNameType();
    }

    getEntriesByName(
      name,
      type = undefined,
    ) {
      webidl.assertBranded(this, Performance);
      const prefix = "Failed to execute 'getEntriesByName' on 'Performance'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });

      if (type !== undefined) {
        type = webidl.converters.DOMString(type, {
          prefix,
          context: "Argument 2",
        });
      }

      return filterByNameType(name, type);
    }

    getEntriesByType(type) {
      webidl.assertBranded(this, Performance);
      const prefix = "Failed to execute 'getEntriesByName' on 'Performance'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      type = webidl.converters.DOMString(type, {
        prefix,
        context: "Argument 1",
      });

      return filterByNameType(undefined, type);
    }

    mark(
      markName,
      markOptions = {},
    ) {
      webidl.assertBranded(this, Performance);
      const prefix = "Failed to execute 'mark' on 'Performance'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      markName = webidl.converters.DOMString(markName, {
        prefix,
        context: "Argument 1",
      });

      markOptions = webidl.converters.PerformanceMarkOptions(markOptions, {
        prefix,
        context: "Argument 2",
      });

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
      startOrMeasureOptions = {},
      endMark = undefined,
    ) {
      webidl.assertBranded(this, Performance);
      const prefix = "Failed to execute 'measure' on 'Performance'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      measureName = webidl.converters.DOMString(measureName, {
        prefix,
        context: "Argument 1",
      });

      startOrMeasureOptions = webidl.converters
        ["DOMString or PerformanceMeasureOptions"](startOrMeasureOptions, {
          prefix,
          context: "Argument 2",
        });

      if (endMark !== undefined) {
        endMark = webidl.converters.DOMString(endMark, {
          prefix,
          context: "Argument 3",
        });
      }

      if (
        startOrMeasureOptions && typeof startOrMeasureOptions === "object" &&
        ObjectKeys(startOrMeasureOptions).length > 0
      ) {
        if (endMark) {
          throw new TypeError("Options cannot be passed with endMark.");
        }
        if (
          !("start" in startOrMeasureOptions) &&
          !("end" in startOrMeasureOptions)
        ) {
          throw new TypeError(
            "A start or end mark must be supplied in options.",
          );
        }
        if (
          "start" in startOrMeasureOptions &&
          "duration" in startOrMeasureOptions &&
          "end" in startOrMeasureOptions
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
        "end" in startOrMeasureOptions
      ) {
        endTime = convertMarkToTimestamp(startOrMeasureOptions.end);
      } else if (
        typeof startOrMeasureOptions === "object" &&
        "start" in startOrMeasureOptions &&
        "duration" in startOrMeasureOptions
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
        "start" in startOrMeasureOptions
      ) {
        startTime = convertMarkToTimestamp(startOrMeasureOptions.start);
      } else if (
        typeof startOrMeasureOptions === "object" &&
        "end" in startOrMeasureOptions &&
        "duration" in startOrMeasureOptions
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
      webidl.assertBranded(this, Performance);
      return now();
    }

    toJSON() {
      webidl.assertBranded(this, Performance);
      return {};
    }

    [customInspect](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: this instanceof Performance,
        keys: [],
      }));
    }

    get [SymbolToStringTag]() {
      return "Performance";
    }
  }
  webidl.configurePrototype(Performance);

  window.__bootstrap.performance = {
    PerformanceEntry,
    PerformanceMark,
    PerformanceMeasure,
    Performance,
    performance: webidl.createBranded(Performance),
  };
})(this);
