// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { DAY, HOUR, MINUTE, SECOND, WEEK } from "./constants.ts";

export type Unit =
  | "milliseconds"
  | "seconds"
  | "minutes"
  | "hours"
  | "days"
  | "weeks"
  | "months"
  | "quarters"
  | "years";

export type DifferenceFormat = Partial<Record<Unit, number>>;

export type DifferenceOptions = {
  units?: Unit[];
};

function calculateMonthsDifference(from: Date, to: Date): number {
  let months = (to.getFullYear() - from.getFullYear()) * 12 +
    (to.getMonth() - from.getMonth());
  if (to.getDate() < from.getDate()) {
    months--;
  }
  return months;
}

/**
 * Returns the difference of the 2 given dates in the given units. If the units
 * are omitted, it returns the difference in the all available units.
 *
 * @example
 * ```ts
 * import { difference } from "https://deno.land/std@$STD_VERSION/datetime/difference.ts";
 *
 * const date0 = new Date("2018-05-14");
 * const date1 = new Date("2020-05-13");
 *
 * difference(date0, date1, { units: ["days", "months", "years"] });
 * // => returns { days: 730, months: 23, years: 1 }
 *
 * difference(date0, date1);
 * // => returns {
 * //   milliseconds: 63072000000,
 * //   seconds: 63072000,
 * //   minutes: 1051200,
 * //   hours: 17520,
 * //   days: 730,
 * //   weeks: 104,
 * //   months: 23,
 * //   quarters: 7,
 * //   years: 1
 * // }
 * ```
 *
 * @param from Year to calculate difference
 * @param to Year to calculate difference with
 * @param options Options for determining how to respond
 */
export function difference(
  from: Date,
  to: Date,
  options?: DifferenceOptions,
): DifferenceFormat {
  [from, to] = from < to ? [from, to] : [to, from];
  const uniqueUnits = options?.units ? [...new Set(options?.units)] : [
    "milliseconds",
    "seconds",
    "minutes",
    "hours",
    "days",
    "weeks",
    "months",
    "quarters",
    "years",
  ];

  const differenceInMs = Math.abs(from.getTime() - to.getTime());

  const differences: DifferenceFormat = {};

  for (const uniqueUnit of uniqueUnits) {
    switch (uniqueUnit) {
      case "milliseconds":
        differences.milliseconds = differenceInMs;
        break;
      case "seconds":
        differences.seconds = Math.floor(differenceInMs / SECOND);
        break;
      case "minutes":
        differences.minutes = Math.floor(differenceInMs / MINUTE);
        break;
      case "hours":
        differences.hours = Math.floor(differenceInMs / HOUR);
        break;
      case "days":
        differences.days = Math.floor(differenceInMs / DAY);
        break;
      case "weeks":
        differences.weeks = Math.floor(differenceInMs / WEEK);
        break;
      case "months":
        differences.months = calculateMonthsDifference(from, to);
        break;
      case "quarters":
        differences.quarters = Math.floor(
          (differences.months !== undefined && differences.months / 3) ||
            calculateMonthsDifference(from, to) / 3,
        );
        break;
      case "years":
        differences.years = Math.floor(
          (differences.months !== undefined && differences.months / 12) ||
            calculateMonthsDifference(from, to) / 12,
        );
        break;
    }
  }

  return differences;
}
