(async () => {
  try {
    await import("does not exist");
  } catch (err) {
    console.log("Caught direct dynamic import error.");
    console.log(err);
  }

  try {
    await import("./indirect_import_error.js");
  } catch (err) {
    console.log("Caught indirect direct dynamic import error.");
    console.log(err);
  }

  try {
    await import("./throws.js");
  } catch (err) {
    console.log("Caught error thrown by dynamically imported module.");
    console.log(err);
  }

  try {
    await import("./indirect_throws.js");
  } catch (err) {
    console.log(
      "Caught error thrown indirectly by dynamically imported module.",
    );
    console.log(err);
  }
})();
