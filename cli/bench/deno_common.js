// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const input = "long-string".repeat(99999);
Deno.bench("b64_rt_long", { n: 100 }, () => {
  atob(btoa(input));
});

Deno.bench("b64_rt_short", { n: 1e6 }, () => {
  atob(btoa("123"));
});

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

Deno.bench("date_now", { n: 5e5 }, () => {
  Date.now();
});

Deno.bench("perf_now", { n: 5e5 }, () => {
  performance.now();
});

{
  // Not too large since we want to measure op-overhead not sys IO
  const dataChunk = new Uint8Array(100);
  const file = Deno.openSync("/dev/null", { write: true });
  Deno.bench("write_null", { n: 5e5 }, () => {
    Deno.writeSync(file.rid, dataChunk);
  });
}

{
  const buf = new Uint8Array(100);
  const file = Deno.openSync("/dev/zero");
  Deno.bench("read_zero", { n: 5e5 }, () => {
    Deno.readSync(file.rid, buf);
  });
}

Deno.bench(
  "read_128k",
  { n: 5e4 },
  () => Deno.readFile("./cli/bench/testdata/128k.bin"),
);

Deno.bench("request_new", { n: 5e5 }, () => new Request("https://deno.land"));

Deno.bench("op_void_sync", { n: 1e7 }, () => Deno.core.opSync("op_void_sync"));

Deno.bench(
  "op_void_async",
  { n: 1e6 },
  async () => {
    const range = new Array(1e3).fill(null);
    for (let i = 0; i < 1e6 / 1e3; i++) {
      await Promise.all(range.map(() => Deno.core.opAsync("op_void_async")));
    }
  },
);
