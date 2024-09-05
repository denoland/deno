const binaryName = Deno.build.os === "windows" ? "deno.exe" : "deno";
const pathSep = Deno.build.os === "windows" ? "\\" : "/";

Deno.mkdirSync("subdir");
Deno.copyFileSync(binaryName, "subdir/" + binaryName);

try {
  const commandResult = new Deno.Command(
    "deno",
    {
      env: { "PATH": Deno.cwd() + pathSep + "subdir" },
      stdout: "inherit",
      stderr: "inherit",
    },
  ).outputSync();

  console.log(commandResult.code);
} catch (err) {
  console.log(err);
}

try {
  const child = Deno.run(
    {
      cmd: ["deno"],
      env: { "PATH": Deno.cwd() + pathSep + "subdir" },
      stdout: "inherit",
      stderr: "inherit",
    },
  );
  console.log((await child.status()).code);
} catch (err) {
  console.log(err);
}
