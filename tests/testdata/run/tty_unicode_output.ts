// Regression test for https://github.com/denoland/deno/issues/32996
// Verifies that Unicode characters (box-drawing, emoji, CJK) are
// written correctly through the TTY WriteStream, matching Node.js
// behavior regardless of the console code page.
import process from "node:process";

// Box-drawing characters (commonly used by CLI tools like vite, clack)
process.stdout.write("┌─────────────┐\n");
process.stdout.write("│ Hello World │\n");
process.stdout.write("└─────────────┘\n");

// Various Unicode ranges
process.stdout.write("Arrows: ◆ ● ▶ ◀\n");
process.stdout.write("CJK: 你好世界\n");
process.stdout.write("Accented: café résumé naïve\n");

console.log("OK");
