import * as cp from "node:child_process";

const child = cp.spawn(
  "cmd.exe",
  ["/d", "/s", "/c"],
  {
    cwd: "test",
  },
);
child.on("exit", (code) => {
  console.log(code);
});
child.on("error", (err) => {
  console.log(err);
});
