import { assertRejects } from "../unit/test_util.ts";

self.onmessage = async (e) => {
  const { oldname, newname } = e.data;
  console.log(oldname, newname);
  await assertRejects(async () => {
    await Deno.symlink(oldname, newname);
  }, Deno.errors.PermissionDenied);
  console.log("posting message");
  self.postMessage("ok");
};
