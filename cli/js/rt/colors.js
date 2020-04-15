// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/colors.ts", ["$deno$/deno.ts"], function (
  exports_12,
  context_12
) {
  "use strict";
  let deno_ts_1, enabled, ANSI_PATTERN;
  const __moduleName = context_12 && context_12.id;
  function code(open, close) {
    return {
      open: `\x1b[${open}m`,
      close: `\x1b[${close}m`,
      regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
    };
  }
  function run(str, code) {
    return enabled
      ? `${code.open}${str.replace(code.regexp, code.open)}${code.close}`
      : str;
  }
  function bold(str) {
    return run(str, code(1, 22));
  }
  exports_12("bold", bold);
  function italic(str) {
    return run(str, code(3, 23));
  }
  exports_12("italic", italic);
  function yellow(str) {
    return run(str, code(33, 39));
  }
  exports_12("yellow", yellow);
  function cyan(str) {
    return run(str, code(36, 39));
  }
  exports_12("cyan", cyan);
  function red(str) {
    return run(str, code(31, 39));
  }
  exports_12("red", red);
  function green(str) {
    return run(str, code(32, 39));
  }
  exports_12("green", green);
  function bgRed(str) {
    return run(str, code(41, 49));
  }
  exports_12("bgRed", bgRed);
  function white(str) {
    return run(str, code(37, 39));
  }
  exports_12("white", white);
  function gray(str) {
    return run(str, code(90, 39));
  }
  exports_12("gray", gray);
  function stripColor(string) {
    return string.replace(ANSI_PATTERN, "");
  }
  exports_12("stripColor", stripColor);
  return {
    setters: [
      function (deno_ts_1_1) {
        deno_ts_1 = deno_ts_1_1;
      },
    ],
    execute: function () {
      enabled = !deno_ts_1.noColor;
      // https://github.com/chalk/ansi-regex/blob/2b56fb0c7a07108e5b54241e8faec160d393aedb/index.js
      ANSI_PATTERN = new RegExp(
        [
          "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:[a-zA-Z\\d]*(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
          "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))",
        ].join("|"),
        "g"
      );
    },
  };
});
