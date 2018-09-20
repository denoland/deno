const benchmarkTypes = ["hello", "relative_import"];

(async () => {
  const data = await (await fetch("./data.json")).json();

  const benchmarkColumns = benchmarkTypes.map(type => [
    type,
    ...data.map(d => {
      const benchmark = d.benchmark[type];
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
