// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const { stdin } = window.__bootstrap.files;
  const { isatty } = window.__bootstrap.tty;
  const LF = "\n".charCodeAt(0);
  const CR = "\r".charCodeAt(0);
  const decoder = new TextDecoder();
  const core = window.Deno.core;

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
        buf.push(CR);
        if (n === null || n === 0) {
          break;
        }
      }
      if (c[0] === LF) {
        break;
      }
      buf.push(c[0]);
    }
    return decoder.decode(new Uint8Array(buf));
  }

  window.__bootstrap.prompt = {
    alert,
    confirm,
    prompt,
  };
})(this);
