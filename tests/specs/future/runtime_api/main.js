console.log("window is", globalThis.window);

try {
  new Deno.FsFile(0);
} catch (error) {
  if (
    error instanceof TypeError &&
    error.message ===
      "`Deno.FsFile` cannot be constructed, use `Deno.open()` or `Deno.openSync()` instead."
  ) {
    console.log("Deno.FsFile constructor is illegal");
  }
}

self.close();
