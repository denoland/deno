// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

window.add_result_callback(({ message, name, stack, status }) => {
  const data = new TextEncoder().encode(
    `${JSON.stringify({ name, status, message, stack })}\n`,
  );
  let bytesWritten = 0;
  while (bytesWritten < data.byteLength) {
    bytesWritten += Deno.stderr.writeSync(data.subarray(bytesWritten));
  }
});

window.add_completion_callback((_tests, harnessStatus) => {
  const data = new TextEncoder().encode(
    `#$#$#${JSON.stringify(harnessStatus)}\n`,
  );
  let bytesWritten = 0;
  while (bytesWritten < data.byteLength) {
    bytesWritten += Deno.stderr.writeSync(data.subarray(bytesWritten));
  }
  Deno.exit(harnessStatus.status === 0 ? 0 : 1);
});
