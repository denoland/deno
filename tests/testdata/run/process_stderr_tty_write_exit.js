import process from "node:process";

process.stderr.write("\0".repeat(32 * 1024), () => {
  Deno.writeTextFileSync(Deno.args[0], "stderr write callback");
});
