console.log("B");
const chalk = (await import("npm:chalk@5")).default;

console.log(chalk.green("C"));

try {
  // Trying to import a devDependency should result in an error
  await import("xo");
} catch (e) {
  console.error("devDependency import failed:", e);
}
