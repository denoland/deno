const url = import.meta.resolve("./mod.ts");
const urls = [] as string[];

// this is hard to reproduce, but doing this will help
for (let i = 0; i < 100; i++) {
  urls.push(url + "#" + i);
}

const results = await Promise.all(urls.map((url) => import(url)));

for (const result of results) {
  result.outputValue();
}
