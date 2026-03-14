Deno.test("ansi escape filtering", () => {
  // Destructive sequences that should be stripped
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
