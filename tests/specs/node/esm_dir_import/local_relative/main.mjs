try {
  await import("./dir");
} catch (err) {
  console.log(err instanceof TypeError);
  console.log(String(err).includes("ERR_UNSUPPORTED_DIR_IMPORT"));
  console.log(String(err).includes("os error"));
}
