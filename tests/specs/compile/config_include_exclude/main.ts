const dataDir = import.meta.dirname + "/data";
const files = Array.from(
  Deno.readDirSync(dataDir).map((entry) => dataDir + "/" + entry.name),
);
files.sort();
for (const file of files) {
  console.log(Deno.readTextFileSync(file).trim());
}
