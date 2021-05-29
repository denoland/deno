console.log((await Deno.emit(new URL("093_root.ts", import.meta.url))).modules);
