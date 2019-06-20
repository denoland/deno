const { makeTempDir, open, run, execPath, remove, readFile } = Deno;

async function main() {
  const tempDir = await makeTempDir();
  const fileName = tempDir + "/redirected_stdio.txt";
  const file = await Deno.open(fileName, "w");

  const p = run({
    args: [execPath, "./tests/subdir/redirected_stdio.ts"],
    stdoutRid: file.rid,
    stderrRid: file.rid
  });

  await p.status();
  p.close();
  file.close();

  const fileContents = await readFile(fileName);
  const decoder = new TextDecoder();
  console.log(decoder.decode(fileContents));

  await remove(tempDir, { recursive: true });
}

main();
