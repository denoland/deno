const { success, stderr } = new Deno.Command(
  Deno.execPath(),
  {
    args: ["run", "-A", "--log-level=debug", "main.tsx"],
  },
).outputSync();
const stderrText = new TextDecoder().decode(stderr);
if (!success) {
  console.error(stderrText);
  throw new Error("Failed to run script.");
}

// create some stability with the output
const lines = stderrText.split("\n")
  .filter((line) => line.includes("Resolved preact from"));
lines.sort();
for (const line of lines) {
  console.error(line);
}
