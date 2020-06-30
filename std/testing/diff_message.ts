import diff, { DiffType } from "./diff.ts";
import { stringify } from "./stringify.ts";
import { green, red, gray, bold } from "../fmt/colors.ts";

export function diffMessage(actual: unknown, expected: unknown): string {
  return [
    "",
    `${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${green(
      bold("Expected")
    )}`,
    "",
    ...diffMessageBody(actual, expected, {
      add: (s: string): string => green(bold(s)),
      remove: (s: string): string => red(bold(s)),
      common: (s: string): string => gray(s),
    }).split("\n"),
    "",
  ]
    .map((line) => "    " + line)
    .join("\n");
}

const identity = <T>(x: T): T => x;
export function diffMessageBody(
  actual: unknown,
  expected: unknown,
  format: {
    add: (s: string) => string;
    remove: (s: string) => string;
    common: (s: string) => string;
  } = {
    add: identity,
    remove: identity,
    common: identity,
  }
): string {
  const getLines = (o: unknown): string[] =>
    stringify(normalise(o)).split("\n");
  const diffResults = diff(getLines(actual), getLines(expected));
  return diffResults
    .map((result) => {
      if (result.type === DiffType.added) {
        return format.add("+ " + result.value);
      } else if (result.type === DiffType.removed) {
        return format.remove("- " + result.value);
      } else {
        return format.common("  " + result.value);
      }
    })
    .join("\n");
}

function normalise(o: unknown): unknown {
  if (Array.isArray(o)) {
    return o.map(normalise);
  } else if (o instanceof Map) {
    return new Map(Array.from(o.entries()).sort());
  } else if (o instanceof Set) {
    return new Set(Array.from(o.values()).sort());
  } else if (typeof o === "object" && o !== null) {
    return Object.entries(o)
      .sort(([a], [b]) => a.localeCompare(b))
      .reduce((result, [key, value]) => {
        return {
          ...result,
          [key]: value,
        };
      }, {});
  } else {
    return o;
  }
}
