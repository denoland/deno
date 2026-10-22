Deno.test("ansi escape filtering", () => {
  // C1 controls (UTF-8 encoded) should be stripped, but the text after the
  // sequence must survive. These lines come first so their exact output can be
  // asserted without a leading wildcard (which would otherwise hide a leak).
  console.log("2Jc1 csi stripped"); // C1 CSI (U+009B) + erase screen
  console.log("0;evil titlec1 osc stripped"); // C1 OSC + BEL term
  // Other destructive C0 controls (BEL, BS, FF, ENQ) should be stripped.
  console.log("c0\x07\x08\x0c\x05 controls stripped");

  // Destructive escape sequences that should be stripped
  console.log("\x1b[2J"); // clear screen
  console.log("\x1b[H"); // cursor home
  console.log("\x1b[10;20H"); // cursor position
  console.log("\x1b[2A"); // cursor up
  console.log("\x1b[K"); // erase line
  console.log("\x1b[?25l"); // hide cursor
  console.log("\x1b[?1049h"); // alt screen
  console.log("\x1bc"); // terminal reset

  // SGR color sequences should pass through
  console.log("\x1b[31mred text\x1b[0m");
  console.log("\x1b[1;32mbold green\x1b[0m");

  // Plain text should pass through
  console.log("plain output");
});
