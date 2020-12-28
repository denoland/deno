// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { opNow } = window.__bootstrap.timers;
  const { cloneValue, illegalConstructorKey } = window.__bootstrap.webUtil;

  const customInspect = Symbol.for("Deno.customInspect");
  let performanceEntries = [];

  function findMostRecent(
    name,
    type,
  ) {
    return performanceEntries
      .slice()
      .reverse()
      .find((entry) => entry.name === name && entry.entryType === type);
  }

  function convertMarkToTimestamp(mark) {
    if (typeof mark === "string") {
      const entry = findMostRecent(mark, "mark");
      if (!entry) {
        throw new SyntaxError(`Cannot find mark: "${mark}".`);
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
    return performanceEntries.filter(
      (entry) =>
        (name ? entry.name === name : true) &&
        (type ? entry.entryType === type : true),
    );
  }

  function now() {
    const res = opNow();
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }

  class PerformanceEntry {
    #name = "";
    #entryType = "";
    #startTime = 0;
    #duration = 0;

    get name() {
      return this.#name;
    }

    get entryType() {
      return this.#entryType;
    }

    get startTime() {
      return this.#startTime;
    }

    get duration() {
      return this.#duration;
    }

    constructor(
      name = null,
      entryType = null,
      startTime = null,
      duration = null,
      key = null,
    ) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      this.#name = name;
      this.#entryType = entryType;
      this.#startTime = startTime;
      this.#duration = duration;
    }

    toJSON() {
      return {
        name: this.#name,
        entryType: this.#entryType,
        startTime: this.#startTime,
        duration: this.#duration,
      };
    }

    [customInspect]() {
      return `${this.constructor.name} { name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
    }
  }

  class PerformanceMark extends PerformanceEntry {
    #detail = null;

    get detail() {
      return this.#detail;
    }

    get entryType() {
      return "mark";
    }

    constructor(
      name,
      { detail = null, startTime = now() } = {},
    ) {
      super(name, "mark", startTime, 0, illegalConstructorKey);
      if (startTime < 0) {
        throw new TypeError("startTime cannot be negative");
      }
      this.#detail = cloneValue(detail);
    }

    toJSON() {
      return {
        name: this.name,
        entryType: this.entryType,
        startTime: this.startTime,
        duration: this.duration,
        detail: this.detail,
      };
    }

    [customInspect]() {
      return this.detail
        ? `${this.constructor.name} {\n  detail: ${
          JSON.stringify(this.detail, null, 2)
        },\n  name: "${this.name}",\n  entryType: "${this.entryType}",\n  startTime: ${this.startTime},\n  duration: ${this.duration}\n}`
        : `${this.constructor.name} { detail: ${this.detail}, name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
    }
  }

  class PerformanceMeasure extends PerformanceEntry {
    #detail = null;

    get detail() {
      return this.#detail;
    }

    get entryType() {
      return "measure";
    }

    constructor(
      name,
      startTime,
      duration,
      detail = null,
      key,
    ) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super(name, "measure", startTime, duration, illegalConstructorKey);
      this.#detail = cloneValue(detail);
    }

    toJSON() {
      return {
        name: this.name,
        entryType: this.entryType,
        startTime: this.startTime,
        duration: this.duration,
        detail: this.detail,
      };
    }

    [customInspect]() {
      return this.detail
        ? `${this.constructor.name} {\n  detail: ${
          JSON.stringify(this.detail, null, 2)
        },\n  name: "${this.name}",\n  entryType: "${this.entryType}",\n  startTime: ${this.startTime},\n  duration: ${this.duration}\n}`
        : `${this.constructor.name} { detail: ${this.detail}, name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
    }
  }

  class Performance {
    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
    }

    clearMarks(markName) {
      if (markName == null) {
        performanceEntries = performanceEntries.filter(
          (entry) => entry.entryType !== "mark",
        );
      } else {
        performanceEntries = performanceEntries.filter(
          (entry) => !(entry.name === markName && entry.entryType === "mark"),
        );
      }
    }

    clearMeasures(measureName) {
      if (measureName == null) {
        performanceEntries = performanceEntries.filter(
          (entry) => entry.entryType !== "measure",
        );
      } else {
        performanceEntries = performanceEntries.filter(
          (entry) =>
            !(entry.name === measureName && entry.entryType === "measure"),
        );
      }
    }

    getEntries() {
      return filterByNameType();
    }

    getEntriesByName(
      name,
      type,
    ) {
      return filterByNameType(name, type);
    }

    getEntriesByType(type) {
      return filterByNameType(undefined, type);
    }

    mark(
      markName,
      options = {},
    ) {
      // 3.1.1.1 If the global object is a Window object and markName uses the
      // same name as a read only attribute in the PerformanceTiming interface,
      // throw a SyntaxError. - not implemented
      const entry = new PerformanceMark(markName, options);
      // 3.1.1.7 Queue entry - not implemented
      performanceEntries.push(entry);
      return entry;
    }

    measure(
      measureName,
      startOrMeasureOptions = {},
      endMark,
    ) {
      if (
        startOrMeasureOptions && typeof startOrMeasureOptions === "object" &&
        Object.keys(startOrMeasureOptions).length > 0
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
      performanceEntries.push(entry);
      return entry;
    }

    now() {
      return now();
    }
  }

  const performance = new Performance(illegalConstructorKey);

  window.__bootstrap.performance = {
    PerformanceEntry,
    PerformanceMark,
    PerformanceMeasure,
    Performance,
    performance,
  };
})(this);
