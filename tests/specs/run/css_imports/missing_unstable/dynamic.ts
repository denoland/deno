console.log(typeof CSSStyleSheet);

function nonAnalyzablePath() {
  return "./style.css";
}

const typeCss = "css";
const { default: sheet } = await import(nonAnalyzablePath(), {
  with: {
    type: typeCss,
  },
});

console.log(sheet);
