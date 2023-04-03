// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
import { isatty } from "ext:runtime/40_tty.js";
import { stdin } from "ext:deno_io/12_io.js";
import { bold, cyan, italic, yellow } from "ext:deno_console/01_colors.js";
const {
  Map,
  Uint8Array,
  StringPrototypeReplace,
  ArrayPrototypeMap,
  ArrayPrototypeJoin,
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
const CR = "\r";
const LF = "\n";
const CRLF = "\r\n";
const NUL = "\0";

function alert(message = "Alert") {
  if (!isatty(stdin.rid)) return;

  core.print(
    `${yellow(bold(message))} [${italic("Press any key to continue")}] `,
  );

  try {
    stdin.setRaw(true);
    stdin.readSync(new Uint8Array(4));
  } finally {
    stdin.setRaw(false);
  }

  core.print(LF);
}

function prompt(message = "Prompt", defaultValue = "") {
  if (!isatty(stdin.rid)) return null;

  return ops.op_read_line_prompt(
    `${yellow(bold(message))} `,
    `${defaultValue}`,
  );
}

function confirm(message = "Confirm") {
  if (!isatty(stdin.rid)) return false;

  const options = [{ val: true, desc: "OK" }, { val: false, desc: "Cancel" }];
  const inputMap = new Map([
    ["y", true],
    ["n", false],
    ["Y", true],
    ["N", false],
    [ESC, false],
    [CTRL_C, false],
    [CTRL_D, false],
    [NUL, false],
  ]);

  core.print(`${yellow(bold(message))}\n`);
  return select(options, inputMap);
}

function select(options, inputMap, selectedIdx = 0) {
  let val = options.at(-1).val;
  try {
    core.print(HIDE_CURSOR);
    stdin.setRaw(true);
    val = _select(options, inputMap, selectedIdx);
  } finally {
    core.print(SHOW_CURSOR);
    stdin.setRaw(false);
  }

  core.print(LF);
  return val;
}

function _select(options, inputMap, selectedIdx = 0) {
  core.print(fmtOptions(options, selectedIdx));

  while (true) {
    const b = new Uint8Array(4);
    stdin.readSync(b);
    const byteString = StringPrototypeReplace(
      StringFromCodePoint(b[0], b[1], b[2], b[3]),
      /\0+$/,
      "",
    ) || "\0";

    if (inputMap.has(byteString)) return inputMap.get(byteString);

    switch (byteString) {
      case UP_ARROW:
        selectedIdx = (options.length + selectedIdx - 1) % options.length;
        break;
      case DOWN_ARROW:
        selectedIdx = (options.length + selectedIdx + 1) % options.length;
        break;
      case CR:
      case LF:
      case CRLF:
        return options[selectedIdx].val;
    }

    core.print(
      `${new Array(options.length).fill(LINE_CLEAR).join(LINE_UP)}${CR}${
        fmtOptions(options, selectedIdx)
      }`,
    );
  }
}

function fmtOptions(options, selectedIdx) {
  return ArrayPrototypeJoin(
    ArrayPrototypeMap(
      options,
      ({ desc }, i) => selectedIdx === i ? cyan(`\u276f ${desc}`) : `  ${desc}`,
    ),
    "\n",
  );
}

export { alert, confirm, prompt };
