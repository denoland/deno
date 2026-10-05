// Local files that are not embedded in the binary must pass the read
// permission check when loaded via `import()` or as a worker, as with
// `deno run`.
const cwd = Deno.cwd().replaceAll("\\", "/");
const toUrl = (path: string) =>
  `file://${cwd.startsWith("/") ? "" : "/"}${cwd}/${path}`;

async function tryImport(path: string, options?: ImportCallOptions) {
  try {
    const mod = await import(toUrl(path), options);
    console.log(path, "loaded", JSON.stringify(mod.default));
  } catch (err) {
    console.log(path, (err as Error).message);
  }
}

await tryImport("allowed/data.json", { with: { type: "json" } });
await tryImport("outside/secret.json", { with: { type: "json" } });
await tryImport("outside/mod.ts");

await new Promise<void>((resolve) => {
  const worker = new Worker(toUrl("outside/worker.ts"), { type: "module" });
  worker.onmessage = (e) => {
    console.log("outside/worker.ts loaded", JSON.stringify(e.data));
    worker.terminate();
    resolve();
  };
  worker.onerror = (e) => {
    e.preventDefault();
    console.log("outside/worker.ts", e.message);
    resolve();
  };
});
