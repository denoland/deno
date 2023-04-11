// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
import { isatty } from "ext:runtime/40_tty.js";
import { stdin } from "ext:deno_io/12_io.js";
import { bold, cyan, italic, yellow } from "ext:deno_console/01_colors.js";
const {
  Array,
  ArrayPrototypeFill,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  Error,
  Map,
  Uint8Array,
  StringFromCodePoint,
} = primordials;

const LINE_UP = "\x1b[1A";
const LINE_CLEAR = "\x1b[2K";
const HIDE_CURSOR = "\x1b[?25l";
const SHOW_CURSOR = "\x1b[?25h";
const UP_ARROW = "\x1b[A";
const DOWN_ARROW = "\x1b[B";
const ESC = "\x1b";
const CTRL_C = "\x03";
const CTRL_D = "\x04";

function alert(message = "Alert") {
  if (!isatty(stdin.rid)) {
    throw new Error("Cannot use `alert` in a non-interactive environment");
  }

  core.print(
    `${yellow(bold(`${message}`))} [${italic("Press any key to continue")}] `,
  );

  try {
    stdin.setRaw(true);
    stdin.readSync(new Uint8Array(1024));
  } finally {
    stdin.setRaw(false);
  }

  core.print("\n");
}

function prompt(message = "Prompt", defaultValue = "") {
  if (!isatty(stdin.rid)) {
    throw new Error("Cannot use `prompt` in a non-interactive environment");
  }

  return ops.op_read_line_prompt(
    `${yellow(bold(`${message}`))} `,
    `${defaultValue}`,
  );
}

function confirm(message = "Confirm") {
  if (!isatty(stdin.rid)) {
    throw new Error("Cannot use `confirm` in a non-interactive environment");
  }

  const options = [{ val: true, desc: "OK" }, { val: false, desc: "Cancel" }];
  const inputMap = new Map([
    ["y", true],
    ["n", false],
    ["Y", true],
    ["N", false],
    [ESC, false],
    [CTRL_C, false],
    [CTRL_D, false],
  ]);

  core.print(`${yellow(bold(`${message}`))}\n`);
  return select(options, inputMap);
}

function select(options, inputMap) {
  let val = options.at(-1).val;
  try {
    core.print(HIDE_CURSOR);
    stdin.setRaw(true);
    val = selectOption(options, inputMap);
  } finally {
    core.print(SHOW_CURSOR);
    stdin.setRaw(false);
  }

  core.print("\n");
  return val;
}

function selectOption(options, inputMap) {
  const selectedIdx = 0;
  core.print(formatOptions(options, selectedIdx));

  while (true) {
    const b = new Uint8Array(1024);
    stdin.readSync(b);
    let byteString = "";

    let i = 0;
    while (b[i]) byteString += StringFromCodePoint(b[i++]);

    if (inputMap.has(byteString)) return inputMap.get(byteString);

    switch (byteString) {
      case UP_ARROW:
        selectedIdx = (options.length + selectedIdx - 1) % options.length;
        break;
      case DOWN_ARROW:
        selectedIdx = (options.length + selectedIdx + 1) % options.length;
        break;
      case "\r":
      case "\n":
      case "\r\n":
        return options[selectedIdx].val;
    }

    core.print(
      `${
        ArrayPrototypeJoin(
          ArrayPrototypeFill(new Array(options.length), LINE_CLEAR),
          LINE_UP,
        )
      }\r${formatOptions(options, selectedIdx)}`,
    );
  }
}

function formatOptions(options, selectedIdx) {
  return ArrayPrototypeJoin(
    ArrayPrototypeMap(
      options,
      ({ desc }, i) => selectedIdx === i ? cyan(`\u276f ${desc}`) : `  ${desc}`,
    ),
    "\n",
  );
}

export { alert, confirm, prompt };
