// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { writeAllSync } from "../../test_util/std/io/util.ts";

window.add_result_callback(({ message, name, stack, status }) => {
  writeAllSync(
    Deno.stderr,
    new TextEncoder().encode(
      `${JSON.stringify({ name, status, message, stack })}\n`,
    ),
  );
});

window.add_completion_callback((_tests, _harnessStatus) => {
  Deno.exit(0);
});
