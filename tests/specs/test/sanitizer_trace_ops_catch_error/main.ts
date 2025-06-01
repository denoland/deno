// https://github.com/denoland/deno/pull/16970
Deno.test("handle thrown error in async function", async () => {
  const dirPath = Deno.makeTempDirSync();
  const filePath = `${dirPath}/file.txt`;
  try {
    await Deno.stat(filePath);
  } catch {
    await Deno.writeTextFile(filePath, "");
  } finally {
    await Deno.remove(filePath);
    await Deno.remove(dirPath);
  }
});
