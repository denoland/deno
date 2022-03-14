const pathname = await Deno.makeTempDir();
Deno.chdir(pathname);
await Deno.remove(pathname);
await Deno.mkdir("X", { recursive: true });
