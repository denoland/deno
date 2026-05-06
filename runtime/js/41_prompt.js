// Copyright 2018-2026 the Deno authors. MIT license.
import { core, primordials } from "ext:core/mod.js";
const { ArrayPrototypePush, StringPrototypeCharCodeAt, Uint8Array } =
  primordials;

const { stdin } = core.loadExtScript("ext:deno_io/12_io.js");

const LF = StringPrototypeCharCodeAt("\n", 0);
const CR = StringPrototypeCharCodeAt("\r", 0);

function alert(message = "Alert") {
  if (!stdin.isTerminal()) {
    return;
  }

  core.print(`${message} [Enter] `, false);

  readLineFromStdinSync();
}

function confirm(message = "Confirm") {
  if (!stdin.isTerminal()) {
    return false;
  }

  core.print(`${message} [y/N] `, false);

  const answer = readLineFromStdinSync();

  return answer === "Y" || answer === "y";
}

function prompt(message = "Prompt", defaultValue) {
  defaultValue ??= "";

  if (!stdin.isTerminal()) {
    return null;
  }

  let formattedMessage = message.length === 0 ? "" : `${message} `;
  if (defaultValue !== "") {
    formattedMessage += `[${defaultValue}] `;
  }

  core.print(formattedMessage, false);

  const answer = readLineFromStdinSync();

  if (answer === "" && defaultValue !== "") {
    return defaultValue;
  }
  return answer === "" ? null : answer;
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
