// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

let total = 5;
let current = "";
const values = {};
const runtime = typeof Deno !== "undefined" ? "deno" : "node";

function bench(fun, count = 100000) {
  if (total === 5) console.log(fun.toString());
  const start = Date.now();
  for (let i = 0; i < count; i++) fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`time ${elapsed} ms rate ${rate}`);
  values[current] = values[current] || [];
  values[current].push(rate);
  if (--total) bench(fun, count);
  else total = 5;
}

let fs;
if (runtime === "node") {
  fs = await import("fs");
}

const getFunction = runtime === "deno"
  ? (name) => {
    current = name;
    return Deno[name];
  }
  : (name) => {
    current = name;
    return fs[name];
  };

const writeFileSync = getFunction("writeFileSync");
writeFileSync("test", new Uint8Array(1024 * 1024), { truncate: true });

const copyFileSync = getFunction("copyFileSync");
bench(() => copyFileSync("test", "test2"), 10000);

const truncateSync = getFunction("truncateSync");
bench(() => truncateSync("test", 0));

const lstatSync = getFunction("lstatSync");
bench(() => lstatSync("test"));

const { uid, gid } = lstatSync("test");

const chownSync = getFunction("chownSync");
bench(() => chownSync("test", uid, gid));

const chmodSync = getFunction("chmodSync");
bench(() => chmodSync("test", 0o666));

// const cwd = getFunction("cwd");
// bench(() => cwd());

// const chdir = getFunction("chdir");
// bench(() => chdir("/"));

const readFileSync = getFunction("readFileSync");
writeFileSync("test", new Uint8Array(1024), { truncate: true });
bench(() => readFileSync("test"));

writeFileSync(
  new URL(`./${runtime}.json`, import.meta.url),
  new TextEncoder().encode(JSON.stringify(values, null, 2)),
  { truncate: true },
);
