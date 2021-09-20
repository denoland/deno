import { assertThrows } from "../unit/test_util.ts";

self.onmessage = async (e) => {
  const { oldname, newname } = e.data;
  assertThrows(() => {
    Deno.symlinkSync(oldname, newname);
  }, Deno.errors.PermissionDenied);
  self.postMessage("ok");
  await new Promise((resolve) => {
    setTimeout(resolve, 500);
  });
};
