try {
  await import("./dir");
} catch (err) {
  console.log(err instanceof TypeError);
  console.log(hasUnsupportedDirImportCode(err));
  console.log(String(err).includes("ERR_UNSUPPORTED_DIR_IMPORT"));
  console.log(String(err).includes("os error"));
}

function hasUnsupportedDirImportCode(err) {
  while (err) {
    if (err.code === "ERR_UNSUPPORTED_DIR_IMPORT") {
      return true;
    }
    err = err.cause;
  }
  return false;
}
