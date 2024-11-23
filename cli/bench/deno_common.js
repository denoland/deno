// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// v8 builtin that's close to the upper bound non-NOPs
Deno.bench("date_now", { n: 5e5 }, () => {
  Date.now();
});

function addJS(a, b) {
  return a + b;
}
Deno.bench("add_js", () => addJS(1, 2));

// A very lightweight op, that should be highly optimizable
Deno.bench("perf_now", { n: 5e5 }, () => {
  performance.now();
});

Deno.bench("open_file_sync", () => {
  using _file = Deno.openSync("./cli/bench/testdata/128k.bin");
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
    file.readSync(buf);
  });
}

{
  // Not too large since we want to measure op-overhead not sys IO
  const dataChunk = new Uint8Array(100);
  const file = Deno.openSync("/dev/null", { write: true });
  Deno.bench("write_null", { n: 5e5 }, () => {
    file.writeSync(dataChunk);
  });
}

Deno.bench(
  "read_128k",
  { n: 5e4 },
  () => Deno.readFile("./cli/bench/testdata/128k.bin"),
);

Deno.bench("request_new", { n: 5e5 }, () => new Request("https://deno.land"));
