const specifier = "./computed_target.js";

for (let attempt = 0; attempt < 2; attempt++) {
  let importError: unknown;
  try {
    await import(specifier);
  } catch (error) {
    importError = error;
  }
  if (importError === undefined) {
    throw new Error("Import unexpectedly succeeded");
  }
  const message = importError instanceof Error
    ? importError.message
    : String(importError);
  if (message.includes("--sloppy-imports")) {
    throw new Error("Unexpected sloppy import suggestion");
  }
}

console.log("no suggestion");
