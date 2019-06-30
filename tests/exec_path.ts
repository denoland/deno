console.log(Deno.execPath);

async function main(): Promise<void> {
  // See https://github.com/denoland/deno/issues/1798
  // Assumes that tests are run from repo root
  // Create path with "/./" segment
  let exePath = Deno.execPath;
  let index = exePath.indexOf("target");
  exePath = exePath.slice(0, index) + "./" + exePath.slice(index);

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

if (Deno.platform.os !== "win") {
  main();
}
