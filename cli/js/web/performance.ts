// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { now as opNow } from "../ops/timers.ts";
import { customInspect, inspect } from "./console.ts";
import { cloneValue, setFunctionName } from "./util.ts";

let performanceEntries: PerformanceEntryList = [];

function findMostRecent(
  name: string,
  type: "mark" | "measure"
): PerformanceEntry | undefined {
  return performanceEntries
    .slice()
    .reverse()
    .find((entry) => entry.name === name && entry.entryType === type);
}

function convertMarkToTimestamp(mark: string | number): number {
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
  name?: string,
  type?: "mark" | "measure"
): PerformanceEntryList {
  return performanceEntries.filter(
    (entry) =>
      (name ? entry.name === name : true) &&
      (type ? entry.entryType === type : true)
  );
}

function now(): number {
  const res = opNow();
  return res.seconds * 1e3 + res.subsecNanos / 1e6;
}

export class PerformanceEntryImpl implements PerformanceEntry {
  #name: string;
  #entryType: string;
  #startTime: number;
  #duration: number;

  get name(): string {
    return this.#name;
  }

  get entryType(): string {
    return this.#entryType;
  }

  get startTime(): number {
    return this.#startTime;
  }

  get duration(): number {
    return this.#duration;
  }

  constructor(
    name: string,
    entryType: string,
    startTime: number,
    duration: number
  ) {
    this.#name = name;
    this.#entryType = entryType;
    this.#startTime = startTime;
    this.#duration = duration;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  toJSON(): any {
    return {
      name: this.#name,
      entryType: this.#entryType,
      startTime: this.#startTime,
      duration: this.#duration,
    };
  }

  [customInspect](): string {
    return `${this.constructor.name} { name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
  }
}

export class PerformanceMarkImpl extends PerformanceEntryImpl
  implements PerformanceMark {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  #detail: any;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get detail(): any {
    return this.#detail;
  }

  get entryType(): "mark" {
    return "mark";
  }

  constructor(
    name: string,
    { detail = null, startTime = now() }: PerformanceMarkOptions = {}
  ) {
    super(name, "mark", startTime, 0);
    if (startTime < 0) {
      throw new TypeError("startTime cannot be negative");
    }
    this.#detail = cloneValue(detail);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  toJSON(): any {
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      detail: this.detail,
    };
  }

  [customInspect](): string {
    return this.detail
      ? `${this.constructor.name} {\n  detail: ${inspect(this.detail, {
          depth: 3,
        })},\n  name: "${this.name}",\n  entryType: "${
          this.entryType
        }",\n  startTime: ${this.startTime},\n  duration: ${this.duration}\n}`
      : `${this.constructor.name} { detail: ${this.detail}, name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
  }
}

export class PerformanceMeasureImpl extends PerformanceEntryImpl
  implements PerformanceMeasure {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  #detail: any;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get detail(): any {
    return this.#detail;
  }

  get entryType(): "measure" {
    return "measure";
  }

  constructor(
    name: string,
    startTime: number,
    duration: number,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    detail: any = null
  ) {
    super(name, "measure", startTime, duration);
    this.#detail = cloneValue(detail);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  toJSON(): any {
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      detail: this.detail,
    };
  }

  [customInspect](): string {
    return this.detail
      ? `${this.constructor.name} {\n  detail: ${inspect(this.detail, {
          depth: 3,
        })},\n  name: "${this.name}",\n  entryType: "${
          this.entryType
        }",\n  startTime: ${this.startTime},\n  duration: ${this.duration}\n}`
      : `${this.constructor.name} { detail: ${this.detail}, name: "${this.name}", entryType: "${this.entryType}", startTime: ${this.startTime}, duration: ${this.duration} }`;
  }
}

export class PerformanceImpl implements Performance {
  clearMarks(markName?: string): void {
    if (markName == null) {
      performanceEntries = performanceEntries.filter(
        (entry) => entry.entryType !== "mark"
      );
    } else {
      performanceEntries = performanceEntries.filter(
        (entry) => !(entry.name === markName && entry.entryType === "mark")
      );
    }
  }

  clearMeasures(measureName?: string): void {
    if (measureName == null) {
      performanceEntries = performanceEntries.filter(
        (entry) => entry.entryType !== "measure"
      );
    } else {
      performanceEntries = performanceEntries.filter(
        (entry) =>
          !(entry.name === measureName && entry.entryType === "measure")
      );
    }
  }

  getEntries(): PerformanceEntryList {
    return filterByNameType();
  }
  getEntriesByName(
    name: string,
    type?: "mark" | "measure"
  ): PerformanceEntryList {
    return filterByNameType(name, type);
  }
  getEntriesByType(type: "mark" | "measure"): PerformanceEntryList {
    return filterByNameType(undefined, type);
  }

  mark(
    markName: string,
    options: PerformanceMarkOptions = {}
  ): PerformanceMark {
    // 3.1.1.1 If the global object is a Window object and markName uses the
    // same name as a read only attribute in the PerformanceTiming interface,
    // throw a SyntaxError. - not implemented
    const entry = new PerformanceMarkImpl(markName, options);
    // 3.1.1.7 Queue entry - not implemented
    performanceEntries.push(entry);
    return entry;
  }

  measure(
    measureName: string,
    options?: PerformanceMeasureOptions
  ): PerformanceMeasure;
  measure(
    measureName: string,
    startMark?: string,
    endMark?: string
  ): PerformanceMeasure;
  measure(
    measureName: string,
    startOrMeasureOptions: string | PerformanceMeasureOptions = {},
    endMark?: string
  ): PerformanceMeasure {
    if (startOrMeasureOptions && typeof startOrMeasureOptions === "object") {
      if (endMark) {
        throw new TypeError("Options cannot be passed with endMark.");
      }
      if (
        !("start" in startOrMeasureOptions) &&
        !("end" in startOrMeasureOptions)
      ) {
        throw new TypeError("A start or end mark must be supplied in options.");
      }
      if (
        "start" in startOrMeasureOptions &&
        "duration" in startOrMeasureOptions &&
        "end" in startOrMeasureOptions
      ) {
        throw new TypeError(
          "Cannot specify start, end, and duration together in options."
        );
      }
    }
    let endTime: number;
    if (endMark) {
      endTime = convertMarkToTimestamp(endMark);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      "end" in startOrMeasureOptions
    ) {
      endTime = convertMarkToTimestamp(startOrMeasureOptions.end!);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      "start" in startOrMeasureOptions &&
      "duration" in startOrMeasureOptions
    ) {
      const start = convertMarkToTimestamp(startOrMeasureOptions.start!);
      const duration = convertMarkToTimestamp(startOrMeasureOptions.duration!);
      endTime = start + duration;
    } else {
      endTime = now();
    }
    let startTime: number;
    if (
      typeof startOrMeasureOptions === "object" &&
      "start" in startOrMeasureOptions
    ) {
      startTime = convertMarkToTimestamp(startOrMeasureOptions.start!);
    } else if (
      typeof startOrMeasureOptions === "object" &&
      "end" in startOrMeasureOptions &&
      "duration" in startOrMeasureOptions
    ) {
      const end = convertMarkToTimestamp(startOrMeasureOptions.end!);
      const duration = convertMarkToTimestamp(startOrMeasureOptions.duration!);
      startTime = end - duration;
    } else if (typeof startOrMeasureOptions === "string") {
      startTime = convertMarkToTimestamp(startOrMeasureOptions);
    } else {
      startTime = 0;
    }
    const entry = new PerformanceMeasureImpl(
      measureName,
      startTime,
      endTime - startTime,
      typeof startOrMeasureOptions === "object"
        ? startOrMeasureOptions.detail ?? null
        : null
    );
    performanceEntries.push(entry);
    return entry;
  }

  now(): number {
    return now();
  }
}

setFunctionName(PerformanceEntryImpl, "PerformanceEntry");
setFunctionName(PerformanceMarkImpl, "PerformanceMark");
setFunctionName(PerformanceMeasureImpl, "PerformanceMeasure");
setFunctionName(PerformanceImpl, "Performance");
