console.log(Deno.execPath);

async function main(): Promise<void> {
  // See https://github.com/denoland/deno/issues/1798
  const p = Deno.run({
    args: ["./target/debug/deno", "eval", "console.log(Deno.execPath);"],
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
