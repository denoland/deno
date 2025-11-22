import fs from "node:fs";
import process from "node:process";
import path from "node:path";

if (process.platform === "win32") {
  const pkgJson = JSON.parse(
    fs.readFileSync(path.join(import.meta.dirname, "package.json")).toString(),
  );
  pkgJson.bin.deno = pkgJson.bin.deno + ".exe";
  fs.writeFileSync(
    path.join(import.meta.dirname, "package.json"),
    JSON.stringify(pkgJson, null, 2),
  );
  fs.copyFileSync(
    process.execPath,
    path.join(import.meta.dirname, "bin", "deno.exe"),
  );
} else {
  fs.copyFileSync(
    process.execPath,
    path.join(import.meta.dirname, "bin", "deno"),
  );
}
