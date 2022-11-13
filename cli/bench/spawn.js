// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

Deno.bench("echo deno", async () => {
  const { code, stdout, stderr } = await Deno.spawn("echo", { args: ["deno"] });
});

Deno.bench("cat 128kb", async () => {
  const { code, stdout, stderr } = await Deno.spawn("cat", {
    args: ["./cli/bench/testdata/128k.bin"],
  });
});
