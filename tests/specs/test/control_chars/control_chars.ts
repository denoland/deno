Deno.test("\x1b[1F\x1b[2Kroot", async (t) => {
  await t.step("\x1b]52;c;bad\x07step", () => {});
});
