// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * The semantic version parser.
 *
 * Adapted directly from [semver](https://github.com/npm/node-semver).
 *
 * ## Versions
 *
 * A "version" is described by the `v2.0.0` specification found at
 * <https://semver.org>.
 *
 * A leading `"="` or `"v"` character is stripped off and ignored.
 *
 * ## Format
 *
 * Semantic versions can be formatted as strings, by default they
 * are formatted as `full`. Below is a diagram showing the various
 * formatting options.
 *
 * ```
 *           ┌───── full
 *       ┌───┴───┐
 *       ├───────── release
 *   ┌───┴───┐   │
 *   ├───────────── primary
 * ┌─┴─┐     │   │
 * 1.2.3-pre.1+b.1
 * │ │ │ └─┬─┘ └┬┘
 * │ │ │   │    └── build
 * │ │ │   └─────── pre
 * │ │ └─────────── patch
 * │ └───────────── minor
 * └─────────────── major
 * ```
 *
 * ## Ranges
 *
 * A `version range` is a set of `comparators` which specify versions that satisfy
 * the range.
 *
 * A `comparator` is composed of an `operator` and a `version`. The set of
 * primitive `operators` is:
 *
 * - `<` Less than
 * - `<=` Less than or equal to
 * - `>` Greater than
 * - `>=` Greater than or equal to
 * - `=` Equal. If no operator is specified, then equality is assumed, so this
 *   operator is optional, but MAY be included.
 *
 * For example, the comparator `>=1.2.7` would match the versions `1.2.7`, `1.2.8`,
 * `2.5.3`, and `1.3.9`, but not the versions `1.2.6` or `1.1.0`.
 *
 * Comparators can be joined by whitespace to form a `comparator set`, which is
 * satisfied by the **intersection** of all of the comparators it includes.
 *
 * A range is composed of one or more comparator sets, joined by `||`. A version
 * matches a range if and only if every comparator in at least one of the
 * `||`-separated comparator sets is satisfied by the version.
 *
 * For example, the range `>=1.2.7 <1.3.0` would match the versions `1.2.7`,
 * `1.2.8`, and `1.2.99`, but not the versions `1.2.6`, `1.3.0`, or `1.1.0`.
 *
 * The range `1.2.7 || >=1.2.9 <2.0.0` would match the versions `1.2.7`, `1.2.9`,
 * and `1.4.6`, but not the versions `1.2.8` or `2.0.0`.
 *
 * ### Prerelease Tags
 *
 * If a version has a prerelease tag (for example, `1.2.3-alpha.3`) then it will
 * only be allowed to satisfy comparator sets if at least one comparator with the
 * same `[major, minor, patch]` tuple also has a prerelease tag.
 *
 * For example, the range `>1.2.3-alpha.3` would be allowed to match the version
 * `1.2.3-alpha.7`, but it would _not_ be satisfied by `3.4.5-alpha.9`, even though
 * `3.4.5-alpha.9` is technically "greater than" `1.2.3-alpha.3` according to the
 * SemVer sort rules. The version range only accepts prerelease tags on the `1.2.3`
 * version. The version `3.4.5` _would_ satisfy the range, because it does not have
 * a prerelease flag, and `3.4.5` is greater than `1.2.3-alpha.7`.
 *
 * The purpose for this behavior is twofold. First, prerelease versions frequently
 * are updated very quickly, and contain many breaking changes that are (by the
 * author"s design) not yet fit for public consumption. Therefore, by default, they
 * are excluded from range matching semantics.
 *
 * Second, a user who has opted into using a prerelease version has clearly
 * indicated the intent to use _that specific_ set of alpha/beta/rc versions. By
 * including a prerelease tag in the range, the user is indicating that they are
 * aware of the risk. However, it is still not appropriate to assume that they have
 * opted into taking a similar risk on the _next_ set of prerelease versions.
 *
 * Note that this behavior can be suppressed (treating all prerelease versions as
 * if they were normal versions, for the purpose of range matching) by setting the
 * `includePrerelease` flag on the options object to any [functions](#functions)
 * that do range matching.
 *
 * #### Prerelease Identifiers
 *
 * The method `.increment` takes an additional `identifier` string argument that
 * will append the value of the string as a prerelease identifier:
 *
 * ```javascript
 * semver.increment(parse("1.2.3"), "prerelease", "beta");
 * // "1.2.4-beta.0"
 * ```
 *
 * ### Build Metadata
 *
 * Build metadata is `.` delimited alpha-numeric string.
 * When parsing a version it is retained on the `build: string[]` field
 * of the semver instance. When incrementing there is an additional parameter that
 * can set the build metadata on the semver instance.
 *
 * ### Advanced Range Syntax
 *
 * Advanced range syntax desugars to primitive comparators in deterministic ways.
 *
 * Advanced ranges may be combined in the same way as primitive comparators using
 * white space or `||`.
 *
 * #### Hyphen Ranges `X.Y.Z - A.B.C`
 *
 * Specifies an inclusive set.
 *
 * - `1.2.3 - 2.3.4` := `>=1.2.3 <=2.3.4`
 *
 * If a partial version is provided as the first version in the inclusive range,
 * then the missing pieces are replaced with zeroes.
 *
 * - `1.2 - 2.3.4` := `>=1.2.0 <=2.3.4`
 *
 * If a partial version is provided as the second version in the inclusive range,
 * then all versions that start with the supplied parts of the tuple are accepted,
 * but nothing that would be greater than the provided tuple parts.
 *
 * - `1.2.3 - 2.3` := `>=1.2.3 <2.4.0`
 * - `1.2.3 - 2` := `>=1.2.3 <3.0.0`
 *
 * #### X-Ranges `1.2.x` `1.X` `1.2.*` `*`
 *
 * Any of `X`, `x`, or `*` may be used to "stand in" for one of the numeric values
 * in the `[major, minor, patch]` tuple.
 *
 * - `*` := `>=0.0.0` (Any version satisfies)
 * - `1.x` := `>=1.0.0 <2.0.0` (Matching major version)
 * - `1.2.x` := `>=1.2.0 <1.3.0` (Matching major and minor versions)
 *
 * A partial version range is treated as an X-Range, so the special character is in
 * fact optional.
 *
 * - `""` (empty string) := `*` := `>=0.0.0`
 * - `1` := `1.x.x` := `>=1.0.0 <2.0.0`
 * - `1.2` := `1.2.x` := `>=1.2.0 <1.3.0`
 *
 * #### Tilde Ranges `~1.2.3` `~1.2` `~1`
 *
 * Allows patch-level changes if a minor version is specified on the comparator.
 * Allows minor-level changes if not.
 *
 * - `~1.2.3` := `>=1.2.3 <1.(2+1).0` := `>=1.2.3 <1.3.0`
 * - `~1.2` := `>=1.2.0 <1.(2+1).0` := `>=1.2.0 <1.3.0` (Same as `1.2.x`)
 * - `~1` := `>=1.0.0 <(1+1).0.0` := `>=1.0.0 <2.0.0` (Same as `1.x`)
 * - `~0.2.3` := `>=0.2.3 <0.(2+1).0` := `>=0.2.3 <0.3.0`
 * - `~0.2` := `>=0.2.0 <0.(2+1).0` := `>=0.2.0 <0.3.0` (Same as `0.2.x`)
 * - `~0` := `>=0.0.0 <(0+1).0.0` := `>=0.0.0 <1.0.0` (Same as `0.x`)
 * - `~1.2.3-beta.2` := `>=1.2.3-beta.2 <1.3.0` Note that prereleases in the
 *   `1.2.3` version will be allowed, if they are greater than or equal to
 *   `beta.2`. So, `1.2.3-beta.4` would be allowed, but `1.2.4-beta.2` would not,
 *   because it is a prerelease of a different `[major, minor, patch]` tuple.
 *
 * #### Caret Ranges `^1.2.3` `^0.2.5` `^0.0.4`
 *
 * Allows changes that do not modify the left-most non-zero element in the
 * `[major, minor, patch]` tuple. In other words, this allows patch and minor
 * updates for versions `1.0.0` and above, patch updates for versions
 * `0.X >=0.1.0`, and _no_ updates for versions `0.0.X`.
 *
 * Many authors treat a `0.x` version as if the `x` were the major
 * "breaking-change" indicator.
 *
 * Caret ranges are ideal when an author may make breaking changes between `0.2.4`
 * and `0.3.0` releases, which is a common practice. However, it presumes that
 * there will _not_ be breaking changes between `0.2.4` and `0.2.5`. It allows for
 * changes that are presumed to be additive (but non-breaking), according to
 * commonly observed practices.
 *
 * - `^1.2.3` := `>=1.2.3 <2.0.0`
 * - `^0.2.3` := `>=0.2.3 <0.3.0`
 * - `^0.0.3` := `>=0.0.3 <0.0.4`
 * - `^1.2.3-beta.2` := `>=1.2.3-beta.2 <2.0.0` Note that prereleases in the
 *   `1.2.3` version will be allowed, if they are greater than or equal to
 *   `beta.2`. So, `1.2.3-beta.4` would be allowed, but `1.2.4-beta.2` would not,
 *   because it is a prerelease of a different `[major, minor, patch]` tuple.
 * - `^0.0.3-beta` := `>=0.0.3-beta <0.0.4` Note that prereleases in the `0.0.3`
 *   version _only_ will be allowed, if they are greater than or equal to `beta`.
 *   So, `0.0.3-pr.2` would be allowed.
 *
 * When parsing caret ranges, a missing `patch` value desugars to the number `0`,
 * but will allow flexibility within that value, even if the major and minor
 * versions are both `0`.
 *
 * - `^1.2.x` := `>=1.2.0 <2.0.0`
 * - `^0.0.x` := `>=0.0.0 <0.1.0`
 * - `^0.0` := `>=0.0.0 <0.1.0`
 *
 * A missing `minor` and `patch` values will desugar to zero, but also allow
 * flexibility within those values, even if the major version is zero.
 *
 * - `^1.x` := `>=1.0.0 <2.0.0`
 * - `^0.x` := `>=0.0.0 <1.0.0`
 *
 * ### Range Grammar
 *
 * Putting all this together, here is a Backus-Naur grammar for ranges, for the
 * benefit of parser authors:
 *
 * ```bnf
 * range-set  ::= range ( logical-or range ) *
 * logical-or ::= ( " " ) * "||" ( " " ) *
 * range      ::= hyphen | simple ( " " simple ) * | ""
 * hyphen     ::= partial " - " partial
 * simple     ::= primitive | partial | tilde | caret
 * primitive  ::= ( "<" | ">" | ">=" | "<=" | "=" ) partial
 * partial    ::= xr ( "." xr ( "." xr qualifier ? )? )?
 * xr         ::= "x" | "X" | "*" | nr
 * nr         ::= "0" | ["1"-"9"] ( ["0"-"9"] ) *
 * tilde      ::= "~" partial
 * caret      ::= "^" partial
 * qualifier  ::= ( "-" pre )? ( "+" build )?
 * pre        ::= parts
 * build      ::= parts
 * parts      ::= part ( "." part ) *
 * part       ::= nr | [-0-9A-Za-z]+
 * ```
 *
 * Note that, since ranges may be non-contiguous, a version might not be greater
 * than a range, less than a range, _or_ satisfy a range! For example, the range
 * `1.2 <1.2.9 || >2.0.0` would have a hole from `1.2.9` until `2.0.0`, so the
 * version `1.2.10` would not be greater than the range (because `2.0.1` satisfies,
 * which is higher), nor less than the range (since `1.2.8` satisfies, which is
 * lower), and it also does not satisfy the range.
 *
 * If you want to know if a version satisfies or does not satisfy a range, use the
 * {@linkcode satisfies} function.
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import {
 *   parse,
 *   parseComparator,
 *   parseRange,
 *   gt,
 *   lt,
 *   format
 * } from "https://deno.land/std@$STD_VERSION/semver/mod.ts";
 *
 * const semver = parse("1.2.3");
 * const range = parseRange("1.x || >=2.5.0 || 5.0.0 - 7.2.3");
 *
 * const s0 = parse("1.2.3");
 * const s1 = parse("9.8.7");
 * gt(s0, s1); // false
 * lt(s0, s1); // true
 *
 * format(semver) // "1.2.3"
 * ```
 *
 * @module
 */
export * from "./cmp.ts";
export * from "./comparator_format.ts";
export * from "./comparator_intersects.ts";
export * from "./comparator_max.ts";
export * from "./comparator_min.ts";
export * from "./compare_build.ts";
export * from "./compare.ts";
export * from "./constants.ts";
export * from "./difference.ts";
export * from "./eq.ts";
export * from "./format.ts";
export * from "./gt.ts";
export * from "./gte.ts";
export * from "./gtr.ts";
export * from "./test_comparator.ts";
export * from "./test_range.ts";
export * from "./increment.ts";
export * from "./is_semver_comparator.ts";
export * from "./is_semver_range.ts";
export * from "./is_semver.ts";
export * from "./lt.ts";
export * from "./lte.ts";
export * from "./ltr.ts";
export * from "./max_satisfying.ts";
export * from "./min_satisfying.ts";
export * from "./neq.ts";
export * from "./outside.ts";
export * from "./parse_comparator.ts";
export * from "./parse_range.ts";
export * from "./parse.ts";
export * from "./range_format.ts";
export * from "./range_intersects.ts";
export * from "./range_max.ts";
export * from "./range_min.ts";
export * from "./rcompare.ts";
export * from "./rsort.ts";
export * from "./sort.ts";
export * from "./types.ts";
export * from "./lte.ts";
export * from "./lte.ts";
export const SEMVER_SPEC_VERSION = "2.0.0";
