import { comment } from "./subdir/comment.ts";

Deno.test("comment", function () {
  comment();
});
