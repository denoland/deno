// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Format milliseconds to time duration.
 *
 * ```ts
 * import { format } from "https://deno.land/std@$STD_VERSION/fmt/duration.ts";
 *
 * // "00:00:01:39:674:000:000"
 * format(99674, { style: "digital" });
 *
 * // "0d 0h 1m 39s 674ms 0µs 0ns"
 * format(99674);
 *
 * // "1m 39s 674ms"
 * format(99674, { ignoreZero: true });
 *
 * // "1 minutes, 39 seconds, 674 milliseconds"
 * format(99674, { style: "full", ignoreZero: true });
 * ```
 * @module
 */

function addZero(num: number, digits: number) {
  return String(num).padStart(digits, "0");
}

interface DurationObject {
  d: number;
  h: number;
  m: number;
  s: number;
  ms: number;
  us: number;
  ns: number;
}

const keyList: Record<keyof DurationObject, string> = {
  d: "days",
  h: "hours",
  m: "minutes",
  s: "seconds",
  ms: "milliseconds",
  us: "microseconds",
  ns: "nanoseconds",
};

/** Parse milliseconds into a duration. */
function millisecondsToDurationObject(ms: number): DurationObject {
  // Duration cannot be negative
  const millis = Math.abs(ms);
  const millisFraction = millis.toFixed(7).slice(-7, -1);
  return {
    d: Math.trunc(millis / 86400000),
    h: Math.trunc(millis / 3600000) % 24,
    m: Math.trunc(millis / 60000) % 60,
    s: Math.trunc(millis / 1000) % 60,
    ms: Math.trunc(millis) % 1000,
    us: +millisFraction.slice(0, 3),
    ns: +millisFraction.slice(3, 6),
  };
}

function durationArray(
  duration: DurationObject,
): { type: keyof DurationObject; value: number }[] {
  return [
    { type: "d", value: duration.d },
    { type: "h", value: duration.h },
    { type: "m", value: duration.m },
    { type: "s", value: duration.s },
    { type: "ms", value: duration.ms },
    { type: "us", value: duration.us },
    { type: "ns", value: duration.ns },
  ];
}

export interface PrettyDurationOptions {
  /**
   * "narrow" for "0d 0h 0m 0s 0ms..."
   * "digital" for "00:00:00:00:000..."
   * "full" for "0 days, 0 hours, 0 minutes,..."
   */
  style: "narrow" | "digital" | "full";
  /**
   * Whether to ignore zero values.
   * With style="narrow" | "full", all zero values are ignored.
   * With style="digital", only values in the ends are ignored.
   */
  ignoreZero: boolean;
}

export function format(
  ms: number,
  options: Partial<PrettyDurationOptions> = {},
): string {
  const opt = Object.assign(
    { style: "narrow", ignoreZero: false },
    options,
  );
  const duration = millisecondsToDurationObject(ms);
  const durationArr = durationArray(duration);
  switch (opt.style) {
    case "narrow": {
      if (opt.ignoreZero) {
        return `${
          durationArr.filter((x) => x.value).map((x) =>
            `${x.value}${x.type === "us" ? "µs" : x.type}`
          )
            .join(" ")
        }`;
      }
      return `${
        durationArr.map((x) => `${x.value}${x.type === "us" ? "µs" : x.type}`)
          .join(" ")
      }`;
    }
    case "full": {
      if (opt.ignoreZero) {
        return `${
          durationArr.filter((x) => x.value).map((x) =>
            `${x.value} ${keyList[x.type]}`
          ).join(", ")
        }`;
      }
      return `${
        durationArr.map((x) => `${x.value} ${keyList[x.type]}`).join(", ")
      }`;
    }
    case "digital": {
      const arr = durationArr.map((x) =>
        ["ms", "us", "ns"].includes(x.type)
          ? addZero(x.value, 3)
          : addZero(x.value, 2)
      );
      if (opt.ignoreZero) {
        let cont = true;
        while (cont) {
          if (!Number(arr[arr.length - 1])) arr.pop();
          else cont = false;
        }
      }
      return arr.join(":");
    }
    default: {
      throw new TypeError(`style must be "narrow", "full", or "digital"!`);
    }
  }
}
