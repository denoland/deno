import rl from "node:readline/promises";
import fs from "node:fs";

const r = rl.createInterface({
  input: fs.createReadStream("main.ts"),
});

for await (const line of r) {
  console.log(line);
}
