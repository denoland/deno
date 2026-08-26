// Exercises all three escaped ranges: C0 (ESC 0x1b, BEL 0x07), DEL (0x7f),
// and C1 (0x9b CSI, 0x80).
Deno.test("\x1b[1F\x1b[2Kroot\x7f\x9b", async (t) => {
  await t.step("\x1b]52;c;bad\x07step\x80", () => {});
});
