// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

window.add_result_callback(({ message, name, stack, status }) => {
  const data = new TextEncoder().encode(
    `${JSON.stringify({ name, status, message, stack })}\n`,
  );
  const bytesWritten = Deno.stderr.writeSync(data);
  if (bytesWritten !== data.byteLength) {
    throw new TypeError("failed to report test result");
  }
});

window.add_completion_callback((_tests, _harnessStatus) => {
  Deno.exit(0);
});
