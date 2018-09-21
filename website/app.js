const benchmarkNames = ["hello", "relative_import"];

(async () => {
  const data = await (await fetch("./data.json")).json();

  const benchmarkColumns = benchmarkNames.map(name => [
    name,
    ...data.map(d => {
      const benchmark = d.benchmark[name];
      return benchmark ? benchmark.mean : 0;
    })
  ]);

  const sha1List = data.map(d => d.sha1);

  c3.generate({
    bindto: "#benchmark-chart",
    data: { columns: benchmarkColumns },
    axis: {
      x: {
        type: "category",
        categories: sha1List
      }
    }
  });
})();
