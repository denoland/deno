// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
((window) => {
  const { stdin, stdout } = window.__bootstrap.files;
  const { isatty } = window.__bootstrap.tty;
  const LF = "\n".charCodeAt(0);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  function alert(message) {
    if (!isatty(stdin.rid)) {
      throw new Error(
        "stdin is not interactive. 'alert' needs stdin to be interactive.",
      );
    }

    if (message) {
      stdout.writeSync(encoder.encode(message));
    }

    readLineFromStdinSync();
  }

  function confirm(message) {
    if (!isatty(stdin.rid)) {
      throw new Error(
        "stdin is not interactive. 'confirm' needs stdin to be interactive.",
      );
    }

    if (message) {
      stdout.writeSync(encoder.encode(message));
    }

    stdout.writeSync(encoder.encode(" [y/N] "));

    const answer = readLineFromStdinSync();

    return answer === "Y" || answer === "y";
  }

  function prompt(message, defaultValue = "") {
    if (!isatty(stdin.rid)) {
      throw new Error(
        "stdin is not interactive. 'prompt' needs stdin to be interactive.",
      );
    }

    if (message) {
      stdout.writeSync(encoder.encode(message));
    }

    stdout.writeSync(encoder.encode(" "));

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
      if (n === 0 || c[0] === LF) {
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
