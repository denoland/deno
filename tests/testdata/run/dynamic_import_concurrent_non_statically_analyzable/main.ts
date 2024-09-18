import * as path from "http://localhost:4545/deno_std/path/mod.ts";

const currentDir = path.dirname(path.fromFileUrl(import.meta.url));
const url = path.toFileUrl(path.join(currentDir, "./mod.ts"));
const urls = [];

// this is hard to reproduce, but doing this will help
for (let i = 0; i < 100; i++) {
  urls.push(url.toString() + "#" + i);
}

const results = await Promise.all(urls.map((url) => import(url)));

for (const result of results) {
  result.outputValue();
}
