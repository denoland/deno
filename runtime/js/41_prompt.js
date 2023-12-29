// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import { isatty } from "ext:runtime/40_tty.js";
import { stdin } from "ext:deno_io/12_io.js";
import { getNoColor } from "ext:deno_console/01_console.js";
const { Uint8Array, StringFromCodePoint } = primordials;

const ESC = "\x1b";
const CTRL_C = "\x03";
const CTRL_D = "\x04";

const bold = ansi(1, 22);
const italic = ansi(3, 23);
const yellow = ansi(33, 0);
function ansi(start, end) {
  return (str) => getNoColor() ? str : `\x1b[${start}m${str}\x1b[${end}m`;
}

function alert(message = "Alert") {
  if (!isatty(stdin.rid)) {
    return;
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
    return null;
  }

  return ops.op_read_line_prompt(
    `${message} `,
    `${defaultValue}`,
  );
}

const inputMap = new primordials.Map([
  ["Y", true],
  ["y", true],
  ["\r", true],
  ["\n", true],
  ["\r\n", true],
  ["N", false],
  ["n", false],
  [ESC, false],
  [CTRL_C, false],
  [CTRL_D, false],
]);

function confirm(message = "Confirm") {
  if (!isatty(stdin.rid)) {
    return false;
  }

  core.print(`${yellow(bold(`${message}`))} [${italic("Y/n")}] `);

  let val = false;
  try {
    stdin.setRaw(true);

    while (true) {
      const b = new Uint8Array(1024);
      stdin.readSync(b);
      let byteString = "";

      let i = 0;
      while (b[i]) byteString += StringFromCodePoint(b[i++]);

      if (inputMap.has(byteString)) {
        val = inputMap.get(byteString);
        break;
      }
    }
  } finally {
    stdin.setRaw(false);
  }

  core.print(`${val ? "y" : "n"}\n`);
  return val;
}

export { alert, confirm, prompt };
