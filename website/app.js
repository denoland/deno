const benchmarkNames = ["hello", "relative_import"];

(async () => {
  const data = await (await fetch("./data.json")).json();

  const execTimeColumns = benchmarkNames.map(name => [
    name,
    ...data.map(d => {
      const benchmark = d.benchmark[name];
      return benchmark ? benchmark.mean : 0;
    })
  ]);

  const binarySizeList = data.map(d => d.binary_size || 0);
  const sha1List = data.map(d => d.sha1);

  c3.generate({
    bindto: "#exec-time-chart",
    data: { columns: execTimeColumns },
    axis: {
      x: {
        type: "category",
        categories: sha1List
      }
    }
  });

  c3.generate({
    bindto: "#binary-size-chart",
    data: { columns: [["binary_size", ...binarySizeList]] },
    axis: {
      x: {
        type: "category",
        categories: sha1List
      },
      y: {
        tick: {
          format: d => formatBytes(d)
        }
      }
    }
  });
})();

// Formats the byte sizes e.g. 19000 -> 18.55KB
// Copied from https://stackoverflow.com/a/18650828
function formatBytes(a, b) {
  if (0 == a) return "0 Bytes";
  var c = 1024,
    d = b || 2,
    e = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"],
    f = Math.floor(Math.log(a) / Math.log(c));
  return parseFloat((a / Math.pow(c, f)).toFixed(d)) + " " + e[f];
}
