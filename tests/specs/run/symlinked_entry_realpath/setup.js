Deno.mkdirSync(".bin");
Deno.symlinkSync("../pkg/bin/cli.js", ".bin/cli");
