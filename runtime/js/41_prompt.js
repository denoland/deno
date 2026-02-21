// Copyright 2018-2026 the Deno authors. MIT license.
import { core, primordials } from "ext:core/mod.js";
const { ArrayPrototypePush, StringPrototypeCharCodeAt, Uint8Array } =
  primordials;

import { stdin } from "ext:deno_io/12_io.js";

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

  // Format the prompt message, showing default value if provided
  let formattedMessage;
  if (message.length === 0) {
    formattedMessage = defaultValue.length > 0 ? `[${defaultValue}] ` : "";
  } else {
    formattedMessage = defaultValue.length > 0
      ? `${message} [${defaultValue}] `
      : `${message} `;
  }

  core.print(formattedMessage, false);

  const answer = readLineFromStdinSync();

  // Return null on EOF, otherwise return the answer or default value
  if (answer === null) {
    return null;
  }

  return answer.length > 0 ? answer : defaultValue;
}

// Reads a line from stdin synchronously.
// Returns the line content (without line ending), or null on EOF.
function readLineFromStdinSync() {
  const c = new Uint8Array(1);
  const buf = [];

  // First read to detect EOF
  const firstRead = stdin.readSync(c);
  if (firstRead === null || firstRead === 0) {
    // EOF before any input
    return null;
  }

  // Handle first byte
  if (c[0] === LF) {
    // Empty line (just Enter pressed)
    return "";
  }
  if (c[0] === CR) {
    const n = stdin.readSync(c);
    if (n === null || n === 0 || c[0] === LF) {
      // Empty line (CR or CRLF)
      return "";
    }
    // CR followed by something other than LF
    ArrayPrototypePush(buf, CR);
  }

  // Add first non-line-ending byte to buffer (if not already handled)
  if (c[0] !== CR && c[0] !== LF) {
    ArrayPrototypePush(buf, c[0]);
  }

  // Read remaining bytes
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
