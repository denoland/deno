// This is to test if Deno would die at 2nd await
// See https://github.com/denoland/deno/issues/919
(async (): Promise<void> => {
  const currDirInfo = await Deno.stat(".");
  const parentDirInfo = await Deno.stat("..");
  console.log(currDirInfo.isDirectory);
  console.log(parentDirInfo.isFile);
})();
