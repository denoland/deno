import { assert } from "../testing/asserts.ts";
import { bold, cyan, red, bgWhite, black } from "./colors.ts";

export interface FormatOptions {
  /** Limit the number of diagnostics formatted.  If the amount exceeds the
   * returned string contains an indication of how many diagnostics that were
   * not formatted. */
  limit?: number;
}

function formatCategoryAndCode(item: Deno.DiagnosticItem): string {
  let category = "";
  switch (item.category) {
    case Deno.DiagnosticCategory.Error:
      category = red("error");
      break;
    case Deno.DiagnosticCategory.Warning:
      category = "warn";
      break;
    case Deno.DiagnosticCategory.Debug:
      category = "debug";
      break;
    case Deno.DiagnosticCategory.Info:
      category = "info";
      break;
  }
  const code = bold(` TS${String(item.code)}`);
  return `${category}${code}: `;
}

function formatDiagnosticMessageChain(
  item: Deno.DiagnosticMessageChain,
  lvl: number
): string {
  let s = `${"  ".repeat(lvl)}${item.message}\n`;
  if (item.next) {
    for (const dmc of item.next) {
      s += formatDiagnosticMessageChain(dmc, lvl + 1);
    }
  }
  return s;
}

function formatMessage(item: Deno.DiagnosticItem, lvl = 0): string {
  if (!item.messageChain) {
    return `${" ".repeat(lvl)}${item.message}`;
  }

  return formatDiagnosticMessageChain(item.messageChain, lvl).slice(0, -1);
}

function formatSourceName(item: Deno.DiagnosticItem): string {
  const { scriptResourceName, lineNumber, startColumn } = item;
  if (!scriptResourceName) {
    return "";
  }
  assert(lineNumber !== undefined);
  assert(startColumn !== undefined);
  return `${scriptResourceName}:${lineNumber + 1}:${startColumn + 1}`;
}

function formatSourceLine(item: Deno.DiagnosticItem, lvl = 0): string {
  const { sourceLine, lineNumber, startColumn, endColumn } = item;
  if (sourceLine === undefined || lineNumber === undefined) {
    return "";
  }
  const line = String(lineNumber + 1);
  const lineColor = black(bgWhite(line));
  const lineLen = line.length;
  const linePadding = black(bgWhite(" ".repeat(lineLen)));
  const underlineChar = endColumn - startColumn <= 1 ? "^" : "~";
  const s = `${" ".repeat(startColumn)}${underlineChar.repeat(
    endColumn - startColumn
  )}`;
  const colorUnderline =
    item.category === Deno.DiagnosticCategory.Error ? red(s) : cyan(s);
  const indent = " ".repeat(lvl);
  return `\n\n${indent}${lineColor} ${sourceLine}\n${indent}${linePadding} ${colorUnderline}\n`;
}

function formatRelatedInfo(item: Deno.DiagnosticItem): string {
  const { relatedInformation } = item;
  if (!relatedInformation) {
    return "";
  }

  let s = "";
  for (const rd of relatedInformation) {
    s += `\n${formatMessage(rd, 2)}\n\n    ► ${formatSourceName(
      rd
    )}${formatSourceLine(rd, 4)}\n`;
  }
  return s;
}

function formatItem(item: Deno.DiagnosticItem): string {
  return `${formatCategoryAndCode(item)}${formatMessage(
    item
  )}\n\n► ${formatSourceName(item)}${formatSourceLine(item)}${formatRelatedInfo(
    item
  )}`;
}

/** Given an array of diagnostic items, return a string which includes color
 * escape codes of those items.
 *
 * Diagnostic items are returned from the `Deno.compile()` and `Deno.bundle()`
 * APIs. For example:
 *
 *       import { formatDiagnostic } from "https://deno.land/std/fmt/diagnostics.ts";
 *
 *       const [diagnostics, out] = await Deno.compile("./foo.ts");
 *
 *       if (diagnostics) {
 *         console.log(formatDiagnostic(diagnostics));
 *       }
 *
 * The function respects the color settings of `std/fmt/colors.ts`.*/
export function formatDiagnostic(
  items: readonly Deno.DiagnosticItem[],
  options: FormatOptions = {}
): string {
  const { limit } = options;
  let output = "";
  const length = limit && items.length > limit ? limit : items.length;
  for (let i = 0; i < length; i++) {
    const item = items[i];
    output += formatItem(item);
  }
  if (items.length > length) {
    const count = items.length - length;
    output += `\n\nAdditional ${count} item${count > 1 ? "(s)" : ""} found.\n`;
  }
  return output;
}
