// Place the real lockfile in a sibling directory and symlink `deno.lock` to
// it, like the Docker setup in https://github.com/denoland/deno/issues/28153
// where the real lockfile lives on a separate volume.
Deno.mkdirSync("real");
Deno.writeTextFileSync("real/deno.lock", '{\n  "version": "5"\n}\n');
Deno.symlinkSync("real/deno.lock", "deno.lock");
