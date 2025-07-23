function nonAnalyzablePath() {
  return "./non_analyzable.txt";
}

function nonAnalyzableUtf8BomPath() {
  return "./non_analyzable_utf8_bom.txt";
}

async function tryRun(action: () => Promise<void>) {
  try {
    await action();
  } catch (err) {
    console.log(err.message);
  }
}

await tryRun(async () => {
  const { default: helloText } = await import("./hello.txt", {
    with: { type: "text" },
  });
  console.log(helloText);
});

await tryRun(async () => {
  const { default: helloBytes } = await import("./hello.txt", {
    with: { type: "bytes" },
  });
  console.log(helloBytes);
});
await tryRun(async () => {
  const nonAnalyzableTypeText = "text";
  const { default: nonAnalyzableText } = await import(nonAnalyzablePath(), {
    with: { type: nonAnalyzableTypeText },
  });
  console.log(nonAnalyzableText);
});

console.log("utf8 bom");
await tryRun(async () => {
  const { default: utf8BomText } = await import("./utf8_bom.txt", {
    with: { type: "text" },
  });
  console.log(utf8BomText, utf8BomText.length);
});
await tryRun(async () => {
  const { default: utf8BomBytes } = await import("./utf8_bom.txt", {
    with: { type: "bytes" },
  });
  console.log(utf8BomBytes);
});

console.log("utf8 bom non-analyzable");
await tryRun(async () => {
  const { default: nonAnalyzableUtf8BomText } = await import(
    nonAnalyzableUtf8BomPath(),
    { with: { type: "text" } }
  );
  console.log(nonAnalyzableUtf8BomText, nonAnalyzableUtf8BomText.length);
});
await tryRun(async () => {
  const { default: nonAnalyzableUtf8BomBytes } = await import(
    nonAnalyzableUtf8BomPath(),
    { with: { type: "bytes" } }
  );
  console.log(nonAnalyzableUtf8BomBytes);
});
