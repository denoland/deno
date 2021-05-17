const dir = Deno.makeTempDirSync();
Deno.chdir(dir);
Deno.removeSync(dir);
