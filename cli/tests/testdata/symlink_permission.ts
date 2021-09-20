import { assertRejects } from "../unit/test_util.ts";

self.onmessage = async (e) => {
  const { oldname, newname } = e.data;
  await assertRejects(async () => {
    await Deno.symlink(oldname, newname);
  }, Deno.errors.PermissionDenied);
  self.postMessage("ok");
  await new Promise((resolve) => {
    setTimeout(resolve, 500);
  });
};
