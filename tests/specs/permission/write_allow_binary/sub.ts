const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";

Deno.writeTextFileSync(binaryName, "");
