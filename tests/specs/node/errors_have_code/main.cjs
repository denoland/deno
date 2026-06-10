try {
  require.resolve("chalk/package.json");
} catch (e) {
  console.log(e.code);
}
