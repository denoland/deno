// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// Deno.serve caused segfault with this example after #16383
// refs:
// - https://github.com/denoland/deno/pull/16383
// - https://github.com/denoland/deno_std/issues/2882
// - revert https://github.com/denoland/deno/pull/16610

const ctl = new AbortController();
Deno.serve(() =>
  new Promise((resolve) => {
    resolve(new Response(new TextEncoder().encode("ok")));
    ctl.abort();
  }), {
  signal: ctl.signal,
  async onListen({ port }) {
    const a = await fetch(`http://localhost:${port}`, {
      method: "POST",
      body: "",
    });
    await a.text();
  },
});
