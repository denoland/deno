// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// A library's unload destructor may call back into JavaScript. When a worker
// is torn down, that call must not enter the worker's disposed isolate.
const dir = await Deno.makeTempDir();
try {
  const source = `${dir}/unload.c`;
  const suffix = Deno.build.os === "darwin" ? "dylib" : "so";
  const library = `${dir}/libunload.${suffix}`;
  await Deno.writeTextFile(
    source,
    `typedef void (*callback_t)(void);
static callback_t stored;
void store_unload_callback(callback_t callback) { stored = callback; }
__attribute__((destructor)) static void on_unload(void) {
  if (stored) stored();
}
`,
  );
  const cc = await new Deno.Command("cc", {
    args: ["-shared", "-fPIC", "-o", library, source],
  }).output();
  if (!cc.success) throw new Error(new TextDecoder().decode(cc.stderr));

  for (let i = 0; i < 20; i++) {
    const worker = new Worker(
      new URL("./worker_unload_callback_worker.ts", import.meta.url).href,
      { type: "module" },
    );
    const stored = new Promise<void>((resolve) => {
      worker.onmessage = () => resolve();
    });
    worker.postMessage({ library, callbackFirst: i % 2 === 1 });
    await stored;
    await worker[Symbol.asyncDispose]();
  }
  console.log("20 unload callbacks did not enter a disposed isolate");
} finally {
  await Deno.remove(dir, { recursive: true });
}
