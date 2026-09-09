self.onmessage = (e) => {
  if (e.data.type === "chdir") {
    // Read cwd first so the change below must invalidate, not just fill.
    Deno.cwd();
    Deno.chdir(e.data.dir);
    // Report the resolved cwd (chdir target may contain symlinks).
    self.postMessage({ type: "chdir-done", cwd: Deno.cwd() });
  } else if (e.data.type === "verify") {
    if (Deno.cwd() !== e.data.cwd) {
      throw new Error(
        `worker sees stale cwd: ${Deno.cwd()} !== ${e.data.cwd}`,
      );
    }
    self.postMessage({ type: "verify-done" });
  }
};
