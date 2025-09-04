// Copyright 2018-2025 the Deno authors. MIT license.

Deno.bench("echo deno", async () => {
  await new Deno.Command("echo", { args: ["deno"] }).output();
});

Deno.bench("cat 128kb", async () => {
  await new Deno.Command("cat", {
    args: ["./cli/bench/testdata/128k.bin"],
  }).output();
});
