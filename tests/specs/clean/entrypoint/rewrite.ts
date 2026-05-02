const path = Deno.args[0];
const contents = Deno.readTextFileSync(path);

const denoDir = Deno.env.getEnv("DENO_DIR");
const cwd = Deno.cwd();

const newContents = contents.replace(cwd, "$CWD").replace(denoDir, "$DENO_DIR");
Deno.writeTextFileSync(path, newContents);
