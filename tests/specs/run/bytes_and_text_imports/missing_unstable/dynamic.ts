function nonAnalyzablePath() {
  return "./data.txt";
}

const typeText = "text";
const { default: data } = await import(nonAnalyzablePath(), {
  with: {
    type: typeText,
  },
});

console.log(data);
