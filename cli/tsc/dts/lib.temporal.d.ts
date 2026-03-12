// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference no-default-lib="true" />

// Adapted from `tc39/proposal-temporal`: https://github.com/tc39/proposal-temporal/blob/main/polyfill/index.d.ts

/**
 * [Specification](https://tc39.es/proposal-temporal/docs/index.html)
 *
 * @category Temporal
 * @experimental
 */
declare namespace Temporal {
  /**
   * @category Temporal
   * @experimental
   */
  export type ComparisonResult = -1 | 0 | 1;
  /**
   * @category Temporal
   * @experimental
   */
  export type RoundingMode =
    | "ceil"
    | "floor"
    | "expand"
    | "trunc"
    | "halfCeil"
    | "halfFloor"
    | "halfExpand"
    | "halfTrunc"
    | "halfEven";

  /**
   * Options for assigning fields using `with()` or entire objects with
   * `from()`.
   *
   * @category Temporal
   * @experimental
   */
  export type AssignmentOptions = {
    /**
     * How to deal with out-of-range values
     *
     * - In `'constrain'` mode, out-of-range values are clamped to the nearest
     *   in-range value.
     * - In `'reject'` mode, out-of-range values will cause the function to
     *   throw a RangeError.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "reject";
  };

  /**
   * Options for assigning fields using `Duration.prototype.with()` or entire
   * objects with `Duration.from()`, and for arithmetic with
   * `Duration.prototype.add()` and `Duration.prototype.subtract()`.
   *
   * @category Temporal
   * @experimental
   */
  export type DurationOptions = {
    /**
     * How to deal with out-of-range values
     *
     * - In `'constrain'` mode, out-of-range values are clamped to the nearest
     *   in-range value.
     * - In `'balance'` mode, out-of-range values are resolved by balancing them
     *   with the next highest unit.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "balance";
  };

  /**
   * Options for conversions of `Temporal.PlainDateTime` to `Temporal.Instant`
   *
   * @category Temporal
   * @experimental
   */
  export type ToInstantOptions = {
    /**
     * Controls handling of invalid or ambiguous times caused by time zone
     * offset changes like Daylight Saving time (DST) transitions.
     *
     * This option is only relevant if a `DateTime` value does not exist in the
     * destination time zone (e.g. near "Spring Forward" DST transitions), or
     * exists more than once (e.g. near "Fall Back" DST transitions).
     *
     * In case of ambiguous or nonexistent times, this option controls what
     * exact time to return:
     * - `'compatible'`: Equivalent to `'earlier'` for backward transitions like
     *   the start of DST in the Spring, and `'later'` for forward transitions
     *   like the end of DST in the Fall. This matches the behavior of legacy
     *   `Date`, of libraries like moment.js, Luxon, or date-fns, and of
     *   cross-platform standards like [RFC 5545
     *   (iCalendar)](https://tools.ietf.org/html/rfc5545).
     * - `'earlier'`: The earlier time of two possible times
     * - `'later'`: The later of two possible times
     * - `'reject'`: Throw a RangeError instead
     *
     * The default is `'compatible'`.
     */
    disambiguation?: "compatible" | "earlier" | "later" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type OffsetDisambiguationOptions = {
    /**
     * Time zone definitions can change. If an application stores data about
     * events in the future, then stored data about future events may become
     * ambiguous, for example if a country permanently abolishes DST. The
     * `offset` option controls this unusual case.
     *
     * - `'use'` always uses the offset (if it's provided) to calculate the
     *   instant. This ensures that the result will match the instant that was
     *   originally stored, even if local clock time is different.
     * - `'prefer'` uses the offset if it's valid for the date/time in this time
     *   zone, but if it's not valid then the time zone will be used as a
     *   fallback to calculate the instant.
     * - `'ignore'` will disregard any provided offset. Instead, the time zone
     *    and date/time value are used to calculate the instant. This will keep
     *    local clock time unchanged but may result in a different real-world
     *    instant.
     * - `'reject'` acts like `'prefer'`, except it will throw a RangeError if
     *   the offset is not valid for the given time zone identifier and
     *   date/time value.
     *
     * If the ISO string ends in 'Z' then this option is ignored because there
     * is no possibility of ambiguity.
     *
     * If a time zone offset is not present in the input, then this option is
     * ignored because the time zone will always be used to calculate the
     * offset.
     *
     * If the offset is not used, and if the date/time and time zone don't
     * uniquely identify a single instant, then the `disambiguation` option will
     * be used to choose the correct instant. However, if the offset is used
     * then the `disambiguation` option will be ignored.
     */
    offset?: "use" | "prefer" | "ignore" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeAssignmentOptions = Partial<
    AssignmentOptions & ToInstantOptions & OffsetDisambiguationOptions
  >;

  /**
   * Options for arithmetic operations like `add()` and `subtract()`
   *
   * @category Temporal
   * @experimental
   */
  export type ArithmeticOptions = {
    /**
     * Controls handling of out-of-range arithmetic results.
     *
     * If a result is out of range, then `'constrain'` will clamp the result to
     * the allowed range, while `'reject'` will throw a RangeError.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type DateUnit = "year" | "month" | "week" | "day";
  /**
   * @category Temporal
   * @experimental
   */
  export type TimeUnit =
    | "hour"
    | "minute"
    | "second"
    | "millisecond"
    | "microsecond"
    | "nanosecond";
  /**
   * @category Temporal
   * @experimental
   */
  export type DateTimeUnit = DateUnit | TimeUnit;

  /**
   * When the name of a unit is provided to a Temporal API as a string, it is
   * usually singular, e.g. 'day' or 'hour'. But plural unit names like 'days'
   * or 'hours' are also accepted.
   *
   * @category Temporal
   * @experimental
   */
  export type PluralUnit<T extends DateTimeUnit> = {
    year: "years";
    month: "months";
    week: "weeks";
    day: "days";
    hour: "hours";
    minute: "minutes";
    second: "seconds";
    millisecond: "milliseconds";
    microsecond: "microseconds";
    nanosecond: "nanoseconds";
  }[T];

  /**
   * @category Temporal
   * @experimental
   */
  export type LargestUnit<T extends DateTimeUnit> = "auto" | T | PluralUnit<T>;
  /**
   * @category Temporal
   * @experimental
   */
  export type SmallestUnit<T extends DateTimeUnit> = T | PluralUnit<T>;
  /**
   * @category Temporal
   * @experimental
   */
  export type TotalUnit<T extends DateTimeUnit> = T | PluralUnit<T>;

  /**
   * Options for outputting precision in toString() on types with seconds
   *
   * @category Temporal
   * @experimental
   */
  export type ToStringPrecisionOptions = {
    fractionalSecondDigits?: "auto" | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
    smallestUnit?: SmallestUnit<
      "minute" | "second" | "millisecond" | "microsecond" | "nanosecond"
    >;

    /**
     * Controls how rounding is performed:
     * - `halfExpand`: Round to the nearest of the values allowed by
     *   `roundingIncrement` and `smallestUnit`. When there is a tie, round up.
     *   This mode is the default.
     * - `ceil`: Always round up, towards the end of time.
     * - `trunc`: Always round down, towards the beginning of time.
     * - `floor`: Also round down, towards the beginning of time. This mode acts
     *   the same as `trunc`, but it's included for consistency with
     *   `Temporal.Duration.round()` where negative values are allowed and
     *   `trunc` rounds towards zero, unlike `floor` which rounds towards
     *   negative infinity which is usually unexpected. For this reason, `trunc`
     *   is recommended for most use cases.
     */
    roundingMode?: RoundingMode;
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type ShowCalendarOption = {
    calendarName?: "auto" | "always" | "never" | "critical";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type CalendarTypeToStringOptions = Partial<
    ToStringPrecisionOptions & ShowCalendarOption
  >;

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeToStringOptions = Partial<
    CalendarTypeToStringOptions & {
      timeZoneName?: "auto" | "never" | "critical";
      offset?: "auto" | "never";
    }
  >;

  /**
   * @category Temporal
   * @experimental
   */
  export type InstantToStringOptions = Partial<
    ToStringPrecisionOptions & {
      timeZone: TimeZoneLike;
    }
  >;

  /**
   * Options to control the result of `until()` and `since()` methods in
   * `Temporal` types.
   *
   * @category Temporal
   * @experimental
   */
  export interface DifferenceOptions<T extends DateTimeUnit> {
    /**
     * The unit to round to. For example, to round to the nearest minute, use
     * `smallestUnit: 'minute'`. This property is optional for `until()` and
     * `since()`, because those methods default behavior is not to round.
     * However, the same property is required for `round()`.
     */
    smallestUnit?: SmallestUnit<T>;

    /**
     * The largest unit to allow in the resulting `Temporal.Duration` object.
     *
     * Larger units will be "balanced" into smaller units. For example, if
     * `largestUnit` is `'minute'` then a two-hour duration will be output as a
     * 120-minute duration.
     *
     * Valid values may include `'year'`, `'month'`, `'week'`, `'day'`,
     * `'hour'`, `'minute'`, `'second'`, `'millisecond'`, `'microsecond'`,
     * `'nanosecond'` and `'auto'`, although some types may throw an exception
     * if a value is used that would produce an invalid result. For example,
     * `hours` is not accepted by `Temporal.PlainDate.prototype.since()`.
     *
     * The default is always `'auto'`, though the meaning of this depends on the
     * type being used.
     */
    largestUnit?: LargestUnit<T>;

    /**
     * Allows rounding to an integer number of units. For example, to round to
     * increments of a half hour, use `{ smallestUnit: 'minute',
     * roundingIncrement: 30 }`.
     */
    roundingIncrement?: number;

    /**
     * Controls how rounding is performed:
     * - `halfExpand`: Round to the nearest of the values allowed by
     *   `roundingIncrement` and `smallestUnit`. When there is a tie, round away
     *   from zero like `ceil` for positive durations and like `floor` for
     *   negative durations.
     * - `ceil`: Always round up, towards the end of time.
     * - `trunc`: Always round down, towards the beginning of time. This mode is
     *   the default.
     * - `floor`: Also round down, towards the beginning of time. This mode acts the
     *   same as `trunc`, but it's included for consistency with
     *   `Temporal.Duration.round()` where negative values are allowed and
     *   `trunc` rounds towards zero, unlike `floor` which rounds towards
     *   negative infinity which is usually unexpected. For this reason, `trunc`
     *   is recommended for most use cases.
     */
    roundingMode?: RoundingMode;
  }

  /**
   * `round` methods take one required parameter. If a string is provided, the
   * resulting `Temporal.Duration` object will be rounded to that unit. If an
   * object is provided, its `smallestUnit` property is required while other
   * properties are optional. A string is treated the same as an object whose
   * `smallestUnit` property value is that string.
   *
   * @category Temporal
   * @experimental
   */
  export type RoundTo<T extends DateTimeUnit> =
    | SmallestUnit<T>
    | {
      /**
       * The unit to round to. For example, to round to the nearest minute,
       * use `smallestUnit: 'minute'`. This option is required. Note that the
       * same-named property is optional when passed to `until` or `since`
       * methods, because those methods do no rounding by default.
       */
      smallestUnit: SmallestUnit<T>;

      /**
       * Allows rounding to an integer number of units. For example, to round to
       * increments of a half hour, use `{ smallestUnit: 'minute',
       * roundingIncrement: 30 }`.
       */
      roundingIncrement?: number;

      /**
       * Controls how rounding is performed:
       * - `halfExpand`: Round to the nearest of the values allowed by
       *   `roundingIncrement` and `smallestUnit`. When there is a tie, round up.
       *   This mode is the default.
       * - `ceil`: Always round up, towards the end of time.
       * - `trunc`: Always round down, towards the beginning of time.
       * - `floor`: Also round down, towards the beginning of time. This mode acts
       *   the same as `trunc`, but it's included for consistency with
       *   `Temporal.Duration.round()` where negative values are allowed and
       *   `trunc` rounds towards zero, unlike `floor` which rounds towards
       *   negative infinity which is usually unexpected. For this reason, `trunc`
       *   is recommended for most use cases.
       */
      roundingMode?: RoundingMode;
    };

  /**
   * The `round` method of the `Temporal.Duration` accepts one required
   * parameter. If a string is provided, the resulting `Temporal.Duration`
   * object will be rounded to that unit. If an object is provided, the
   * `smallestUnit` and/or `largestUnit` property is required, while other
   * properties are optional. A string parameter is treated the same as an
   * object whose `smallestUnit` property value is that string.
   *
   * @category Temporal
   * @experimental
   */
  export type DurationRoundTo =
    | SmallestUnit<DateTimeUnit>
    | (
      & (
        | {
          /**
           * The unit to round to. For example, to round to the nearest
           * minute, use `smallestUnit: 'minute'`. This property is normally
           * required, but is optional if `largestUnit` is provided and not
           * undefined.
           */
          smallestUnit: SmallestUnit<DateTimeUnit>;

          /**
           * The largest unit to allow in the resulting `Temporal.Duration`
           * object.
           *
           * Larger units will be "balanced" into smaller units. For example,
           * if `largestUnit` is `'minute'` then a two-hour duration will be
           * output as a 120-minute duration.
           *
           * Valid values include `'year'`, `'month'`, `'week'`, `'day'`,
           * `'hour'`, `'minute'`, `'second'`, `'millisecond'`,
           * `'microsecond'`, `'nanosecond'` and `'auto'`.
           *
           * The default is `'auto'`, which means "the largest nonzero unit in
           * the input duration". This default prevents expanding durations to
           * larger units unless the caller opts into this behavior.
           *
           * If `smallestUnit` is larger, then `smallestUnit` will be used as
           * `largestUnit`, superseding a caller-supplied or default value.
           */
          largestUnit?: LargestUnit<DateTimeUnit>;
        }
        | {
          /**
           * The unit to round to. For example, to round to the nearest
           * minute, use `smallestUnit: 'minute'`. This property is normally
           * required, but is optional if `largestUnit` is provided and not
           * undefined.
           */
          smallestUnit?: SmallestUnit<DateTimeUnit>;

          /**
           * The largest unit to allow in the resulting `Temporal.Duration`
           * object.
           *
           * Larger units will be "balanced" into smaller units. For example,
           * if `largestUnit` is `'minute'` then a two-hour duration will be
           * output as a 120-minute duration.
           *
           * Valid values include `'year'`, `'month'`, `'week'`, `'day'`,
           * `'hour'`, `'minute'`, `'second'`, `'millisecond'`,
           * `'microsecond'`, `'nanosecond'` and `'auto'`.
           *
           * The default is `'auto'`, which means "the largest nonzero unit in
           * the input duration". This default prevents expanding durations to
           * larger units unless the caller opts into this behavior.
           *
           * If `smallestUnit` is larger, then `smallestUnit` will be used as
           * `largestUnit`, superseding a caller-supplied or default value.
           */
          largestUnit: LargestUnit<DateTimeUnit>;
        }
      )
      & {
        /**
         * Allows rounding to an integer number of units. For example, to round
         * to increments of a half hour, use `{ smallestUnit: 'minute',
         * roundingIncrement: 30 }`.
         */
        roundingIncrement?: number;

        /**
         * Controls how rounding is performed:
         * - `halfExpand`: Round to the nearest of the values allowed by
         *   `roundingIncrement` and `smallestUnit`. When there is a tie, round
         *   away from zero like `ceil` for positive durations and like `floor`
         *   for negative durations. This mode is the default.
         * - `ceil`: Always round towards positive infinity. For negative
         *   durations this option will decrease the absolute value of the
         *   duration which may be unexpected. To round away from zero, use
         *   `ceil` for positive durations and `floor` for negative durations.
         * - `trunc`: Always round down towards zero.
         * - `floor`: Always round towards negative infinity. This mode acts the
         *   same as `trunc` for positive durations but for negative durations
         *   it will increase the absolute value of the result which may be
         *   unexpected. For this reason, `trunc` is recommended for most "round
         *   down" use cases.
         */
        roundingMode?: RoundingMode;

        /**
         * The starting point to use for rounding and conversions when
         * variable-length units (years, months, weeks depending on the
         * calendar) are involved. This option is required if any of the
         * following are true:
         * - `unit` is `'week'` or larger units
         * - `this` has a nonzero value for `weeks` or larger units
         *
         * This value must be either a `Temporal.PlainDateTime`, a
         * `Temporal.ZonedDateTime`, or a string or object value that can be
         * passed to `from()` of those types. Examples:
         * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
         * - `'2020-01-01'`
         * - `Temporal.PlainDate.from('2020-01-01')`
         *
         * `Temporal.ZonedDateTime` will be tried first because it's more
         * specific, with `Temporal.PlainDateTime` as a fallback.
         *
         * If the value resolves to a `Temporal.ZonedDateTime`, then operation
         * will adjust for DST and other time zone transitions. Otherwise
         * (including if this option is omitted), then the operation will ignore
         * time zone transitions and all days will be assumed to be 24 hours
         * long.
         */
        relativeTo?:
          | Temporal.PlainDateTime
          | Temporal.ZonedDateTime
          | PlainDateTimeLike
          | ZonedDateTimeLike
          | string;
      }
    );

  /**
   * Options to control behavior of `Duration.prototype.total()`
   *
   * @category Temporal
   * @experimental
   */
  export type DurationTotalOf =
    | TotalUnit<DateTimeUnit>
    | {
      /**
       * The unit to convert the duration to. This option is required.
       */
      unit: TotalUnit<DateTimeUnit>;

      /**
       * The starting point to use when variable-length units (years, months,
       * weeks depending on the calendar) are involved. This option is required if
       * any of the following are true:
       * - `unit` is `'week'` or larger units
       * - `this` has a nonzero value for `weeks` or larger units
       *
       * This value must be either a `Temporal.PlainDateTime`, a
       * `Temporal.ZonedDateTime`, or a string or object value that can be passed
       * to `from()` of those types. Examples:
       * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
       * - `'2020-01-01'`
       * - `Temporal.PlainDate.from('2020-01-01')`
       *
       * `Temporal.ZonedDateTime` will be tried first because it's more
       * specific, with `Temporal.PlainDateTime` as a fallback.
       *
       * If the value resolves to a `Temporal.ZonedDateTime`, then operation will
       * adjust for DST and other time zone transitions. Otherwise (including if
       * this option is omitted), then the operation will ignore time zone
       * transitions and all days will be assumed to be 24 hours long.
       */
      relativeTo?:
        | Temporal.ZonedDateTime
        | Temporal.PlainDateTime
        | ZonedDateTimeLike
        | PlainDateTimeLike
        | string;
    };

  /**
   * Options to control behavior of `Duration.compare()`
   *
   * @category Temporal
   * @experimental
   */
  export interface DurationArithmeticOptions {
    /**
     * The starting point to use when variable-length units (years, months,
     * weeks depending on the calendar) are involved. This option is required if
     * either of the durations has a nonzero value for `weeks` or larger units.
     *
     * This value must be either a `Temporal.PlainDateTime`, a
     * `Temporal.ZonedDateTime`, or a string or object value that can be passed
     * to `from()` of those types. Examples:
     * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
     * - `'2020-01-01'`
     * - `Temporal.PlainDate.from('2020-01-01')`
     *
     * `Temporal.ZonedDateTime` will be tried first because it's more
     * specific, with `Temporal.PlainDateTime` as a fallback.
     *
     * If the value resolves to a `Temporal.ZonedDateTime`, then operation will
     * adjust for DST and other time zone transitions. Otherwise (including if
     * this option is omitted), then the operation will ignore time zone
     * transitions and all days will be assumed to be 24 hours long.
     */
    relativeTo?:
      | Temporal.ZonedDateTime
      | Temporal.PlainDateTime
      | ZonedDateTimeLike
      | PlainDateTimeLike
      | string;
  }

  /**
   * Options to control behaviour of `ZonedDateTime.prototype.getTimeZoneTransition()`
   *
   * @category Temporal
   * @experimental
   */
  export type TransitionDirection = "next" | "previous" | {
    direction: "next" | "previous";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type DurationLike = {
    years?: number;
    months?: number;
    weeks?: number;
    days?: number;
    hours?: number;
    minutes?: number;
    seconds?: number;
    milliseconds?: number;
    microseconds?: number;
    nanoseconds?: number;
  };

  /**
   * A `Temporal.Duration` represents an immutable duration of time which can be
   * used in date/time arithmetic.
   *
   * See https://tc39.es/proposal-temporal/docs/duration.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class Duration {
    static from(
      item: Temporal.Duration | DurationLike | string,
    ): Temporal.Duration;
    static compare(
      one: Temporal.Duration | DurationLike | string,
      two: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): ComparisonResult;
    constructor(
      years?: number,
      months?: number,
      weeks?: number,
      days?: number,
      hours?: number,
      minutes?: number,
      seconds?: number,
      milliseconds?: number,
      microseconds?: number,
      nanoseconds?: number,
    );
    readonly sign: -1 | 0 | 1;
    readonly blank: boolean;
    readonly years: number;
    readonly months: number;
    readonly weeks: number;
    readonly days: number;
    readonly hours: number;
    readonly minutes: number;
    readonly seconds: number;
    readonly milliseconds: number;
    readonly microseconds: number;
    readonly nanoseconds: number;
    negated(): Temporal.Duration;
    abs(): Temporal.Duration;
    with(durationLike: DurationLike): Temporal.Duration;
    add(
      other: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): Temporal.Duration;
    subtract(
      other: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): Temporal.Duration;
    round(roundTo: DurationRoundTo): Temporal.Duration;
    total(totalOf: DurationTotalOf): number;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ToStringPrecisionOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.Duration";
  }

  /**
   * A `Temporal.Instant` is an exact point in time, with a precision in
   * nanoseconds. No time zone or calendar information is present. Therefore,
   * `Temporal.Instant` has no concept of days, months, or even hours.
   *
   * For convenience of interoperability, it internally uses nanoseconds since
   * the {@link https://en.wikipedia.org/wiki/Unix_time|Unix epoch} (midnight
   * UTC on January 1, 1970). However, a `Temporal.Instant` can be created from
   * any of several expressions that refer to a single point in time, including
   * an {@link https://en.wikipedia.org/wiki/ISO_8601|ISO 8601 string} with a
   * time zone offset such as '2020-01-23T17:04:36.491865121-08:00'.
   *
   * See https://tc39.es/proposal-temporal/docs/instant.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class Instant {
    static fromEpochMilliseconds(epochMilliseconds: number): Temporal.Instant;
    static fromEpochNanoseconds(epochNanoseconds: bigint): Temporal.Instant;
    static from(item: Temporal.Instant | string): Temporal.Instant;
    static compare(
      one: Temporal.Instant | string,
      two: Temporal.Instant | string,
    ): ComparisonResult;
    constructor(epochNanoseconds: bigint);
    readonly epochMilliseconds: number;
    readonly epochNanoseconds: bigint;
    equals(other: Temporal.Instant | string): boolean;
    add(
      durationLike:
        | Omit<
          Temporal.Duration | DurationLike,
          "years" | "months" | "weeks" | "days"
        >
        | string,
    ): Temporal.Instant;
    subtract(
      durationLike:
        | Omit<
          Temporal.Duration | DurationLike,
          "years" | "months" | "weeks" | "days"
        >
        | string,
    ): Temporal.Instant;
    until(
      other: Temporal.Instant | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.Instant | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Instant;
    toZonedDateTimeISO(tzLike: TimeZoneLike): Temporal.ZonedDateTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: InstantToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.Instant";
  }

  /**
   * Any of these types can be passed to Temporal methods instead of a calendar ID.
   *
   * @category Temporal
   * @experimental
   */
  export type CalendarLike =
    | string
    | ZonedDateTime
    | PlainDateTime
    | PlainDate
    | PlainYearMonth
    | PlainMonthDay;

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainDateLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainDate` represents a calendar date. "Calendar date" refers to the
   * concept of a date as expressed in everyday usage, independent of any time
   * zone. For example, it could be used to represent an event on a calendar
   * which happens during the whole day no matter which time zone it's happening
   * in.
   *
   * See https://tc39.es/proposal-temporal/docs/date.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainDate {
    static from(
      item: Temporal.PlainDate | PlainDateLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainDate;
    static compare(
      one: Temporal.PlainDate | PlainDateLike | string,
      two: Temporal.PlainDate | PlainDateLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      isoDay: number,
      calendar?: string,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly daysInWeek: number;
    readonly daysInYear: number;
    readonly daysInMonth: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(other: Temporal.PlainDate | PlainDateLike | string): boolean;
    with(
      dateLike: PlainDateLike,
      options?: AssignmentOptions,
    ): Temporal.PlainDate;
    withCalendar(calendar: CalendarLike): Temporal.PlainDate;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDate;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDate;
    until(
      other: Temporal.PlainDate | PlainDateLike | string,
      options?: DifferenceOptions<"year" | "month" | "week" | "day">,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainDate | PlainDateLike | string,
      options?: DifferenceOptions<"year" | "month" | "week" | "day">,
    ): Temporal.Duration;
    toPlainDateTime(
      temporalTime?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.PlainDateTime;
    toZonedDateTime(
      timeZoneAndTime:
        | string
        | {
          timeZone: TimeZoneLike;
          plainTime?: Temporal.PlainTime | PlainTimeLike | string;
        },
    ): Temporal.ZonedDateTime;
    toPlainYearMonth(): Temporal.PlainYearMonth;
    toPlainMonthDay(): Temporal.PlainMonthDay;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainDate";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainDateTimeLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainDateTime` represents a calendar date and wall-clock time, with
   * a precision in nanoseconds, and without any time zone. Of the Temporal
   * classes carrying human-readable time information, it is the most general
   * and complete one. `Temporal.PlainDate`, `Temporal.PlainTime`, `Temporal.PlainYearMonth`,
   * and `Temporal.PlainMonthDay` all carry less information and should be used when
   * complete information is not required.
   *
   * See https://tc39.es/proposal-temporal/docs/datetime.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainDateTime {
    static from(
      item: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainDateTime;
    static compare(
      one: Temporal.PlainDateTime | PlainDateTimeLike | string,
      two: Temporal.PlainDateTime | PlainDateTimeLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      isoDay: number,
      hour?: number,
      minute?: number,
      second?: number,
      millisecond?: number,
      microsecond?: number,
      nanosecond?: number,
      calendar?: string,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly daysInWeek: number;
    readonly daysInYear: number;
    readonly daysInMonth: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(other: Temporal.PlainDateTime | PlainDateTimeLike | string): boolean;
    with(
      dateTimeLike: PlainDateTimeLike,
      options?: AssignmentOptions,
    ): Temporal.PlainDateTime;
    withPlainTime(
      timeLike?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.PlainDateTime;
    withCalendar(calendar: CalendarLike): Temporal.PlainDateTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDateTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDateTime;
    until(
      other: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.PlainDateTime;
    toZonedDateTime(
      tzLike: TimeZoneLike,
      options?: ToInstantOptions,
    ): Temporal.ZonedDateTime;
    toPlainDate(): Temporal.PlainDate;
    toPlainTime(): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: CalendarTypeToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainDateTime";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainMonthDayLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainMonthDay` represents a particular day on the calendar, but
   * without a year. For example, it could be used to represent a yearly
   * recurring event, like "Bastille Day is on the 14th of July."
   *
   * See https://tc39.es/proposal-temporal/docs/monthday.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainMonthDay {
    static from(
      item: Temporal.PlainMonthDay | PlainMonthDayLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainMonthDay;
    constructor(
      isoMonth: number,
      isoDay: number,
      calendar?: string,
      referenceISOYear?: number,
    );
    readonly monthCode: string;
    readonly day: number;
    readonly calendarId: string;
    equals(other: Temporal.PlainMonthDay | PlainMonthDayLike | string): boolean;
    with(
      monthDayLike: PlainMonthDayLike,
      options?: AssignmentOptions,
    ): Temporal.PlainMonthDay;
    toPlainDate(year: { year: number }): Temporal.PlainDate;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainMonthDay";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainTimeLike = {
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
  };

  /**
   * A `Temporal.PlainTime` represents a wall-clock time, with a precision in
   * nanoseconds, and without any time zone. "Wall-clock time" refers to the
   * concept of a time as expressed in everyday usage — the time that you read
   * off the clock on the wall. For example, it could be used to represent an
   * event that happens daily at a certain time, no matter what time zone.
   *
   * `Temporal.PlainTime` refers to a time with no associated calendar date; if you
   * need to refer to a specific time on a specific day, use
   * `Temporal.PlainDateTime`. A `Temporal.PlainTime` can be converted into a
   * `Temporal.PlainDateTime` by combining it with a `Temporal.PlainDate` using the
   * `toPlainDateTime()` method.
   *
   * See https://tc39.es/proposal-temporal/docs/plaintime.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainTime {
    static from(
      item: Temporal.PlainTime | PlainTimeLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainTime;
    static compare(
      one: Temporal.PlainTime | PlainTimeLike | string,
      two: Temporal.PlainTime | PlainTimeLike | string,
    ): ComparisonResult;
    constructor(
      hour?: number,
      minute?: number,
      second?: number,
      millisecond?: number,
      microsecond?: number,
      nanosecond?: number,
    );
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    equals(other: Temporal.PlainTime | PlainTimeLike | string): boolean;
    with(
      timeLike: Temporal.PlainTime | PlainTimeLike,
      options?: AssignmentOptions,
    ): Temporal.PlainTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainTime;
    until(
      other: Temporal.PlainTime | PlainTimeLike | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainTime | PlainTimeLike | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ToStringPrecisionOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainTime";
  }

  /**
   * Any of these types can be passed to Temporal methods instead of a time zone ID.
   *
   * @category Temporal
   * @experimental
   */
  export type TimeZoneLike = string | ZonedDateTime;

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainYearMonthLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainYearMonth` represents a particular month on the calendar. For
   * example, it could be used to represent a particular instance of a monthly
   * recurring event, like "the June 2019 meeting".
   *
   * See https://tc39.es/proposal-temporal/docs/yearmonth.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainYearMonth {
    static from(
      item: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainYearMonth;
    static compare(
      one: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      two: Temporal.PlainYearMonth | PlainYearMonthLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      calendar?: string,
      referenceISODay?: number,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly calendarId: string;
    readonly daysInMonth: number;
    readonly daysInYear: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
    ): boolean;
    with(
      yearMonthLike: PlainYearMonthLike,
      options?: AssignmentOptions,
    ): Temporal.PlainYearMonth;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainYearMonth;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainYearMonth;
    until(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: DifferenceOptions<"year" | "month">,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: DifferenceOptions<"year" | "month">,
    ): Temporal.Duration;
    toPlainDate(day: { day: number }): Temporal.PlainDate;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainYearMonth";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
    offset?: string;
    timeZone?: TimeZoneLike;
    calendar?: CalendarLike;
  };

  /**
   * @category Temporal
   * @experimental
   */
  export class ZonedDateTime {
    static from(
      item: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: ZonedDateTimeAssignmentOptions,
    ): ZonedDateTime;
    static compare(
      one: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      two: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
    ): ComparisonResult;
    constructor(epochNanoseconds: bigint, timeZone: string, calendar?: string);
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    readonly timeZoneId: string;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly hoursInDay: number;
    readonly daysInWeek: number;
    readonly daysInMonth: number;
    readonly daysInYear: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    readonly offsetNanoseconds: number;
    readonly offset: string;
    readonly epochMilliseconds: number;
    readonly epochNanoseconds: bigint;
    equals(other: Temporal.ZonedDateTime | ZonedDateTimeLike | string): boolean;
    with(
      zonedDateTimeLike: ZonedDateTimeLike,
      options?: ZonedDateTimeAssignmentOptions,
    ): Temporal.ZonedDateTime;
    withPlainTime(
      timeLike?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.ZonedDateTime;
    withCalendar(calendar: CalendarLike): Temporal.ZonedDateTime;
    withTimeZone(timeZone: TimeZoneLike): Temporal.ZonedDateTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.ZonedDateTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.ZonedDateTime;
    until(
      other: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: Temporal.DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: Temporal.DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.ZonedDateTime;
    startOfDay(): Temporal.ZonedDateTime;
    getTimeZoneTransition(
      direction: TransitionDirection,
    ): Temporal.ZonedDateTime | null;
    toInstant(): Temporal.Instant;
    toPlainDateTime(): Temporal.PlainDateTime;
    toPlainDate(): Temporal.PlainDate;
    toPlainTime(): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ZonedDateTimeToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.ZonedDateTime";
  }

  /**
   * The `Temporal.Now` object has several methods which give information about
   * the current date, time, and time zone.
   *
   * See https://tc39.es/proposal-temporal/docs/now.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export const Now: {
    /**
     * Get the exact system date and time as a `Temporal.Instant`.
     *
     * This method gets the current exact system time, without regard to
     * calendar or time zone. This is a good way to get a timestamp for an
     * event, for example. It works like the old-style JavaScript `Date.now()`,
     * but with nanosecond precision instead of milliseconds.
     *
     * Note that a `Temporal.Instant` doesn't know about time zones. For the
     * exact time in a specific time zone, use `Temporal.Now.zonedDateTimeISO`
     * or `Temporal.Now.zonedDateTime`.
     */
    instant: () => Temporal.Instant;

    /**
     * Get the current calendar date and clock time in a specific time zone,
     * using the ISO 8601 calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    zonedDateTimeISO: (tzLike?: TimeZoneLike) => Temporal.ZonedDateTime;

    /**
     * Get the current date and clock time in a specific time zone, using the
     * ISO 8601 calendar.
     *
     * Note that the `Temporal.PlainDateTime` type does not persist the time zone,
     * but retaining the time zone is required for most time-zone-related use
     * cases. Therefore, it's usually recommended to use
     * `Temporal.Now.zonedDateTimeISO` instead of this function.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainDateTimeISO: (tzLike?: TimeZoneLike) => Temporal.PlainDateTime;

    /**
     * Get the current date in a specific time zone, using the ISO 8601
     * calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainDateISO: (tzLike?: TimeZoneLike) => Temporal.PlainDate;

    /**
     * Get the current clock time in a specific time zone, using the ISO 8601 calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainTimeISO: (tzLike?: TimeZoneLike) => Temporal.PlainTime;

    /**
     * Get the identifier of the environment's current time zone.
     *
     * This method gets the identifier of the current system time zone. This
     * will usually be a named
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone}.
     */
    timeZoneId: () => string;

    readonly [Symbol.toStringTag]: "Temporal.Now";
  };
}

/**
 * @category Temporal
 * @experimental
 */
interface Date {
  toTemporalInstant(): Temporal.Instant;
}
