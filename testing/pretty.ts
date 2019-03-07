// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { equal } from "./asserts.ts";
import { red, green, white, gray, bold } from "../colors/mod.ts";
import diff, { DiffType, DiffResult } from "./diff.ts";
import { format } from "./format.ts";

const CAN_NOT_DISPLAY = "[Cannot display]";

function createStr(v: unknown): string {
  try {
    return format(v);
  } catch (e) {
    return red(CAN_NOT_DISPLAY);
  }
}

function createColor(diffType: DiffType): (s: string) => string {
  switch (diffType) {
    case DiffType.added:
      return (s: string) => green(bold(s));
    case DiffType.removed:
      return (s: string) => red(bold(s));
    default:
      return white;
  }
}

function createSign(diffType: DiffType): string {
  switch (diffType) {
    case DiffType.added:
      return "+   ";
    case DiffType.removed:
      return "-   ";
    default:
      return "    ";
  }
}

function buildMessage(diffResult: ReadonlyArray<DiffResult<string>>): string[] {
  const messages = [];
  messages.push("");
  messages.push("");
  messages.push(
    `    ${gray(bold("[Diff]"))} ${red(bold("Left"))} / ${green(bold("Right"))}`
  );
  messages.push("");
  messages.push("");
  diffResult.forEach((result: DiffResult<string>) => {
    const c = createColor(result.type);
    messages.push(c(`${createSign(result.type)}${result.value}`));
  });
  messages.push("");

  return messages;
}

export function assertEquals(
  actual: unknown,
  expected: unknown,
  msg?: string
): void {
  if (equal(actual, expected)) {
    return;
  }
  let message = "";
  const actualString = createStr(actual);
  const expectedString = createStr(expected);
  try {
    const diffResult = diff(
      actualString.split("\n"),
      expectedString.split("\n")
    );
    message = buildMessage(diffResult).join("\n");
  } catch (e) {
    message = `\n${red(CAN_NOT_DISPLAY)} + \n\n`;
  }
  if (msg) {
    message = msg;
  }
  throw new Error(message);
}
