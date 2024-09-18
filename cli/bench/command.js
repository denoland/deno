// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

Deno.bench("echo deno", async () => {
  await new Deno.Command("echo", { args: ["deno"] }).output();
});

Deno.bench("cat 128kb", async () => {
  await new Deno.Command("cat", {
    args: ["./cli/bench/testdata/128k.bin"],
  }).output();
});
