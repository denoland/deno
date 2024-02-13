// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { ANY } from "./constants.ts";
import type { FormatStyle, SemVer } from "./types.ts";

function formatNumber(value: number) {
  if (value === Number.POSITIVE_INFINITY) {
    return "∞";
  } else if (value === Number.NEGATIVE_INFINITY) {
    return "⧞";
  } else {
    return value.toFixed(0);
  }
}

/**
 * Format a SemVer object into a string.
 *
 * If any number is NaN then NaN will be printed.
 *
 * If any number is positive or negative infinity then '∞' or '⧞' will be printed instead.
 *
 * @param semver The semantic version to format
 * @returns The string representation of a semantic version.
 */
export function format(semver: SemVer, style: FormatStyle = "full") {
  if (semver === ANY) {
    return "*";
  }

  const major = formatNumber(semver.major);
  const minor = formatNumber(semver.minor);
  const patch = formatNumber(semver.patch);
  const pre = semver.prerelease.join(".");
  const build = semver.build.join(".");

  const primary = `${major}.${minor}.${patch}`;
  const release = [primary, pre].filter((v) => v).join("-");
  const full = [release, build].filter((v) => v).join("+");
  switch (style) {
    case "full":
      return full;
    case "release":
      return release;
    case "primary":
      return primary;
    case "build":
      return build;
    case "pre":
      return pre;
    case "patch":
      return patch;
    case "minor":
      return minor;
    case "major":
      return major;
  }
}
