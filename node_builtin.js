import fs from "node:fs";

try {
  const data = fs.readFileSync("./node_builtin.ts", "utf8");
  console.log(data);
} catch (err) {
  console.error(err);
}
