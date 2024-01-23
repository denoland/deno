const PREVIOUS_RELEASE = Deno.args[0];
if (!PREVIOUS_RELEASE) {
  throw new Error("Missing previous release tag");
}

const cmd = new Deno.Command("git", {
  args: ["log", `${PREVIOUS_RELEASE}..`, "--oneline"],
});
const { code, stdout, stderr } = await cmd.output();

const stdoutText = new TextDecoder().decode(stdout);
const stderrText = new TextDecoder().decode(stderr);

console.log(code, stderrText, stdoutText);
if (stderrText) {
  throw new Error(stderrText);
}

const lines = stdoutText.split("\n").filter((line) => line.length > 0).map(
  (line) => {
    // Drop the hash and the first space
    const firstSpace = line.indexOf(" ");
    const messageAndGhRef = line.slice(firstSpace + 1);
    const ghRefIndexIndex = messageAndGhRef.indexOf(" (#");
    const message = messageAndGhRef.slice(0, ghRefIndexIndex);
    const ghRef = messageAndGhRef.slice(ghRefIndexIndex + 3).split(")")[0];
    const output =
      `- ${message} (https://github.com/denoland/deno/pull/${ghRef})`;
    console.log("output", output);
    return output;
  },
).toSorted();

Deno.writeTextFileSync("RELEASE_NOTES.txt", lines.join("\n"));
