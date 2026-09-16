// Copyright 2018-2026 the Deno authors. MIT license.

let iteration = 0;
while (true) {
  await import(`./data.json?iteration=${iteration}`, {
    with: { type: "json" },
  });
  postMessage(iteration);
  iteration++;
}
