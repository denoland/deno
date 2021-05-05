// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
window.add_result_callback(({ message, name, stack, status }) => {
  Deno.writeAllSync(
    Deno.stderr,
    new TextEncoder().encode(
      `${JSON.stringify({ name, status, message, stack })}\n`,
    ),
  );
});

window.add_completion_callback((tests, harnessStatus) => {
  Deno.exit(0);
});
