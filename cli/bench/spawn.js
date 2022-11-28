// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

Deno.bench("echo deno", async () => {
  await Deno.spawn("echo", { args: ["deno"] });
});

Deno.bench("cat 128kb", async () => {
  await Deno.spawn("cat", {
    args: ["./cli/bench/testdata/128k.bin"],
  });
});
