const pathUrl = "http://localhost:4545/deno_std/path/mod.ts";
const fsUrl = "http://localhost:4545/deno_std/fs/mod.ts";
const [path, fs] = await Promise.all([
  await import(pathUrl),
  await import(fsUrl),
]);

const currentFilePath = path.fromFileUrl(import.meta.url);
console.log(currentFilePath);
console.log(fs.existsSync(currentFilePath));
