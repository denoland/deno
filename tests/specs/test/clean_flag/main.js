import { emptyDir } from "@std/fs/empty-dir";

const DIR = "./coverage";
const COMMAND = new Deno.Command(Deno.execPath(), {
  args: ["test", "--coverage", "--clean", "--coverage-raw-data-only"],
  stdout: "null",
});

async function getCoverageFiles() {
  return await Array.fromAsync(Deno.readDir(DIR), ({ name }) => name);
}

await emptyDir(DIR);
await COMMAND.output();
const files1 = new Set(await getCoverageFiles());

await COMMAND.output();
const files2 = new Set(await getCoverageFiles());

console.log(files1.size === files2.size);
console.log(files1.intersection(files2).size === 0);
await emptyDir(DIR);
await Deno.remove(DIR);
