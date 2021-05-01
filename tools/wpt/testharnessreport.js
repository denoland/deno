// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
window.add_result_callback(({ message, name, stack, status }) => {
  Deno.writeAllSync(
    Deno.stderr,
    new TextEncoder().encode(
      `${JSON.stringify({ name, status, message, stack })}\n`,
    ),
  );
});

// TODO(kt3k): Enable the below hook and timers test when #10445 is fixed
// ref: https://github.com/denoland/deno/issues/10445
/*
window.add_completion_callback((tests, harnessStatus) => {
  Deno.exit(0);
});
*/

globalThis.document = {
  // document.body shim for FileAPI/file/File-constructor.any.js test
  body: {
    toString() {
      return '[object HTMLBodyElement]';
    }
  }
};
