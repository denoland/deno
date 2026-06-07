import * as fs from "node:fs";
import * as fsPromises from "node:fs/promises";

try {
  Deno.readTextFileSync("./deno.json");
  console.log("loaded");
} catch (err) {
  console.log(err instanceof Deno.errors.NotFound);
}

try {
  console.log(fs.existsSync("./deno.json"));
} catch {
  console.log("failed");
}
try {
  console.log(
    await new Promise((resolve) => {
      fs.exists("./deno.json", resolve);
    }),
  );
} catch {
  console.log("failed");
}

try {
  await fsPromises.stat("./deno.json");
} catch (err) {
  console.log(
    err instanceof Error && "code" in err &&
      err.code === "ENOENT",
  );
}
