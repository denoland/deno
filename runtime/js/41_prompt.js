// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { core } from "deno:core/01_core.js";
import { primordials } from "deno:core/00_primordials.js";
const { isatty } = globalThis.__bootstrap.tty;
const { stdin } = globalThis.__bootstrap.files;
const { ArrayPrototypePush, StringPrototypeCharCodeAt, Uint8Array } =
  primordials;
const LF = StringPrototypeCharCodeAt("\n", 0);
const CR = StringPrototypeCharCodeAt("\r", 0);

function alert(message = "Alert") {
  if (!isatty(stdin.rid)) {
    return;
  }

  core.print(`${message} [Enter] `, false);

  readLineFromStdinSync();
}

function confirm(message = "Confirm") {
  if (!isatty(stdin.rid)) {
    return false;
  }

  core.print(`${message} [y/N] `, false);

  const answer = readLineFromStdinSync();

  return answer === "Y" || answer === "y";
}

function prompt(message = "Prompt", defaultValue) {
  defaultValue ??= null;

  if (!isatty(stdin.rid)) {
    return null;
  }

  core.print(`${message} `, false);

  if (defaultValue) {
    core.print(`[${defaultValue}] `, false);
  }

  return readLineFromStdinSync() || defaultValue;
}

function readLineFromStdinSync() {
  const c = new Uint8Array(1);
  const buf = [];

  while (true) {
    const n = stdin.readSync(c);
    if (n === null || n === 0) {
      break;
    }
    if (c[0] === CR) {
      const n = stdin.readSync(c);
      if (c[0] === LF) {
        break;
      }
      ArrayPrototypePush(buf, CR);
      if (n === null || n === 0) {
        break;
      }
    }
    if (c[0] === LF) {
      break;
    }
    ArrayPrototypePush(buf, c[0]);
  }
  return core.decode(new Uint8Array(buf));
}

export { alert, confirm, prompt };
