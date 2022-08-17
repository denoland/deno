// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// v8 builtin that's close to the upper bound non-NOPs
Deno.bench("date_now", { n: 5e5 }, () => {
  Date.now();
});

// Void ops measure op-overhead
Deno.bench(
  "op_void_sync",
  { n: 1e7 },
  () => Deno.core.ops.op_void_sync(),
);

Deno.bench(
  "op_void_async",
  { n: 1e6 },
  () => Deno.core.opAsync("op_void_async"),
);

// A very lightweight op, that should be highly optimizable
Deno.bench("perf_now", { n: 5e5 }, () => {
  performance.now();
});

// A common "language feature", that should be fast
// also a decent representation of a non-trivial JSON-op
{
  let i = 0;
  Deno.bench("url_parse", { n: 5e4 }, () => {
    new URL(`http://www.google.com/${i}`);
    i++;
  });
}

Deno.bench("blob_text_large", { n: 100 }, () => {
  new Blob([input]).text();
});

const input = "long-string".repeat(99999);
Deno.bench("b64_rt_long", { n: 100 }, () => {
  atob(btoa(input));
});

Deno.bench("b64_rt_short", { n: 1e6 }, () => {
  atob(btoa("123"));
});

{
  const buf = new Uint8Array(100);
  const file = Deno.openSync("/dev/zero");
  Deno.bench("read_zero", { n: 5e5 }, () => {
    Deno.readSync(file.rid, buf);
  });
}

{
  // Not too large since we want to measure op-overhead not sys IO
  const dataChunk = new Uint8Array(100);
  const file = Deno.openSync("/dev/null", { write: true });
  Deno.bench("write_null", { n: 5e5 }, () => {
    Deno.writeSync(file.rid, dataChunk);
  });
}

Deno.bench(
  "read_128k",
  { n: 5e4 },
  () => Deno.readFile("./cli/bench/testdata/128k.bin"),
);

Deno.bench("request_new", { n: 5e5 }, () => new Request("https://deno.land"));
