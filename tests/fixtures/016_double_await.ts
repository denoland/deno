import * as deno from "deno";

// This is to test if Deno would die at 2nd await
// See https://github.com/denoland/deno/issues/919
(async () => {
  const currDirInfo = await deno.stat(".");
  const parentDirInfo = await deno.stat("..");
  console.log(currDirInfo.isDirectory());
  console.log(parentDirInfo.isFile());
})();
