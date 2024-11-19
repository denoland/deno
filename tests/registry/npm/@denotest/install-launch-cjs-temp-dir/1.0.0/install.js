const tempDir = Deno.makeTempDirSync();
try {
  // should work requiring these because this was launched via a node binary entrypoint
  Deno.writeTextFileSync(`${tempDir}/index.js`, "module.exports = require('./other');");
  Deno.writeTextFileSync(`${tempDir}/other.js`, "module.exports = (a, b) => a + b;");
  const add = require(`${tempDir}/index.js`);
  if (add(1, 2) !== 3) {
    throw new Error("FAILED");
  }
} finally {
  Deno.removeSync(tempDir, { recursive: true });
}
