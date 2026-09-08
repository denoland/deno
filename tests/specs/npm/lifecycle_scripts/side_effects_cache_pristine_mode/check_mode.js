// Asserts the install did NOT mutate the pristine global-cache copy of the
// package's bin entrypoint (`main.js`).
//
// The fixture ships `main.js` non-executable (mode 0664). Setting up
// `node_modules/.bin` chmods the bin entrypoint to make it executable. Because a
// package's `.deno` directory is hardlinked to this pristine extraction on
// Linux, that chmod previously flipped the execute bit on the SHARED inode,
// corrupting the global cache. With the copy-before-bin fix the chmod operates
// on a private copy, so this pristine file stays non-executable.
const npmDir = `${Deno.env.get("DENO_DIR")}/npm`;

// Find the pristine `main.js` under `<pkg>/1.0.0/`, skipping the built
// side-effects variant directories (named `*.build_<hash>`).
function findPristineMainJs(dir) {
  for (const entry of Deno.readDirSync(dir)) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory) {
      if (entry.name.includes(".build_")) continue;
      const found = findPristineMainJs(path);
      if (found) return found;
    } else if (
      entry.name === "main.js" &&
      dir.includes("lifecycle-scripts-simple") &&
      dir.endsWith("1.0.0")
    ) {
      return path;
    }
  }
  return null;
}

const pristine = findPristineMainJs(npmDir);
if (!pristine) {
  throw new Error("pristine main.js not found in global cache");
}

if (Deno.build.os === "windows") {
  // Unix file modes don't apply on Windows; the corruption the fix prevents is
  // an execute-bit flip, which is a Unix-only concept.
  console.log("pristine main.js mode preserved");
} else {
  const mode = Deno.statSync(pristine).mode & 0o777;
  if ((mode & 0o111) !== 0) {
    throw new Error(
      `pristine global-cache main.js became executable (mode ${
        mode.toString(8)
      }) — the .bin chmod corrupted the shared cache`,
    );
  }
  console.log("pristine main.js mode preserved");
}
