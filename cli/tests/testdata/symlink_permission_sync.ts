import { assertThrows } from "../unit/test_util.ts";

self.onmessage = ({ oldname, newname }) => {
  assertThrows(() => {
    Deno.symlinkSync(oldname, newname);
  }, Deno.errors.PermissionDenied);
  self.postMessage("ok");
};
