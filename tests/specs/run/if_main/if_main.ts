if (import.meta.main) {
  console.log("main");
} else {
  console.log("import.meta.url", import.meta.url);
  throw Error("not main");
}
