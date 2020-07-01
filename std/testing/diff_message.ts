import diff, { DiffType } from "./diff.ts";
import { green, red, gray, bold } from "../fmt/colors.ts";

export function diffMessage(actual: unknown, expected: unknown): string {
  return [
    "",
    `    ${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${green(
      bold("Expected")
    )}`,
    "",
    ...diffMessageBody(actual, expected).split("\n"),
    "",
  ].join("\n");
}

export function format(
  o: unknown,
  options: { pretty: boolean } = { pretty: true }
): string {
  if (typeof o === "string") {
    return `"${o.replace(/(?=["\\])/g, "\\")}"`;
  } else {
    return globalThis.Deno
      ? Deno.inspect(o, {
          depth: Number.MAX_SAFE_INTEGER,
          pretty: options.pretty,
        })
      : String(o);
  }
}

export function diffMessageBody(actual: unknown, expected: unknown): string {
  const getLines = (o: unknown): string[] => {
    return format(o).split("\n");
  };
  const diffResults = diff(getLines(actual), getLines(expected));
  return diffResults
    .map((result) => {
      if (result.type === DiffType.added) {
        return bold(green("+   " + result.value));
      } else if (result.type === DiffType.removed) {
        return bold(red("-   " + result.value));
      } else {
        return bold(gray("    " + result.value));
      }
    })
    .join("\n");
}
