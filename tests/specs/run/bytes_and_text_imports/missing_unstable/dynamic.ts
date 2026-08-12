function nonAnalyzablePath() {
  return "./data.txt";
}

const typeBytes = "bytes";
const { default: data } = await import(nonAnalyzablePath(), {
  with: {
    type: typeBytes,
  },
});

console.log(data);
