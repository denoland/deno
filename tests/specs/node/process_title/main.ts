import process from "node:process";

const mode = Deno.args[0];

switch (mode) {
  case "default":
    // Default title should be the execPath
    console.log(
      process.title === process.execPath
        ? "PASS"
        : `FAIL: expected execPath, got ${process.title}`,
    );
    break;
  case "set":
    // Setting title should work
    process.title = "my-custom-title";
    console.log(
      process.title === "my-custom-title"
        ? "PASS"
        : `FAIL: expected my-custom-title, got ${process.title}`,
    );
    break;
  case "node_options":
    // Title should come from NODE_OPTIONS --title=...
    console.log(`title=${process.title}`);
    break;
}
