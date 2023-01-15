// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

let [total, count] = typeof Deno !== "undefined"
  ? Deno.args
  : [process.argv[2], process.argv[3]];

total = total ? parseInt(total, 0) : 3;
count = count ? parseInt(count, 10) : 100;

async function bench(name, fun) {
  if (total === 3) console.log(name);
  const start = Date.now();
  for (let i = 0; i < count; i++) await fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);

  if (--total) await bench(null, fun);
  else total = 3;
}

function compress(buffer) {
  const cs = new CompressionStream("gzip");
  const writer = cs.writable.getWriter();
  writer.write(buffer);
  writer.close();
  return (new Response(cs.readable).arrayBuffer());
}

function decompress(buffer) {
  const ds = new DecompressionStream("gzip");
  const writer = ds.writable.getWriter();
  writer.write(buffer);
  writer.close();
  return (new Response(ds.readable).arrayBuffer());
}

const data = typeof Deno !== "undefined"
  ? Deno.readFileSync("cli/bench/gzip/uncompressed.png")
  : require("fs").readFileSync("cli/bench/gzip/uncompressed.png");

console.log("uncompressed size", data.byteLength);
if (typeof Deno !== "undefined") {
  (async () => {
    const compressed = await compress(data);
    console.log("compressed size", compressed.byteLength, "\n");
    await bench("roundtrip", async () => {
      await decompress(await compress(data));
    });
    await bench(`gzip`, async () => {
      await compress(data);
    });
    await bench(`gunzip`, async () => {
      await decompress(compressed);
    });
  })();
} else {
  const { gzipSync, gunzipSync } = typeof Bun !== "undefined"
    ? Bun
    : require("zlib");
  const compressed = gzipSync(data);
  console.log("compressed size", compressed.byteLength, "\n");
  (async () => {
    await bench("roundtrip", () => {
      gunzipSync(gzipSync(data));
    });

    await bench(`gzipSync`, () => {
      gzipSync(data);
    });

    await bench(`gunzipSync`, () => {
      gunzipSync(compressed);
    });
  })();
}
