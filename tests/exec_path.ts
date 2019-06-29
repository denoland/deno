console.log(Deno.execPath);

async function main(): Promise<void> {
  // See https://github.com/denoland/deno/issues/1798
  // Assumes that tests are run from repo root
  let exePath = Deno.cwd() + "/./target/debug/deno";

  if (Deno.platform.os == "win") {
    exePath += ".exe";
  }

  const p = Deno.run({
    args: [exePath, "eval", "console.log(Deno.execPath);"],
    stdout: "piped"
  });

  await p.status();
  const output = await p.output();
  const textOutput = new TextDecoder().decode(output);

  if (textOutput.indexOf("/./") > -1) {
    throw Error("Exec path contains non-normalized components");
  }
}

main();
