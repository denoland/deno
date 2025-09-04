fetch("http://localhost:4545/wasm/math.wasm").then(async (response) => {
  if (!response.ok) {
    throw new Error(`Failed to fetch WASM module: ${response.statusText}`);
  }
  using file = Deno.openSync("math.wasm", { write: true, create: true });
  await response.body!.pipeTo(file.writable);
});
