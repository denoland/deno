try {
  await import("../target/module.js");
  throw new Error("Import unexpectedly succeeded");
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  if (!message.includes("--sloppy-imports")) {
    throw new Error("Missing sloppy import extension suggestion");
  }
  console.log("extension suggestion");
}

try {
  await import("../target/directory");
  throw new Error("Import unexpectedly succeeded");
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  if (!message.includes("index.ts")) {
    throw new Error("Missing sloppy import directory suggestion");
  }
  console.log("directory suggestion");
}
