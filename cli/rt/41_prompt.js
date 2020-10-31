// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
((window) => {
  const { stdin, stdout } = window.__bootstrap.files;
  const { isatty } = window.__bootstrap.tty;
  const LF = "\n".charCodeAt(0);
  const CR = "\r".charCodeAt(0);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  function alert(message = "Alert") {
    if (!isatty(stdin.rid)) {
      return;
    }

    stdout.writeSync(encoder.encode(`${message} [Enter] `));

    readLineFromStdinSync();
  }

  function confirm(message = "Confirm") {
    if (!isatty(stdin.rid)) {
      return false;
    }

    stdout.writeSync(encoder.encode(`${message} [y/N] `));

    const answer = readLineFromStdinSync();

    return answer === "Y" || answer === "y";
  }

  function prompt(message = "Prompt", defaultValue) {
    defaultValue ??= null;

    if (!isatty(stdin.rid)) {
      return null;
    }

    stdout.writeSync(encoder.encode(`${message} `));

    if (defaultValue) {
      stdout.writeSync(encoder.encode(`[${defaultValue}] `));
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
