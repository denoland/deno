// Copyright 2018 the Deno authors. All rights reserved. MIT license.

export async function getJson(path) {
  return (await fetch(path)).json();
}

const benchmarkNames = ["hello", "relative_import"];
export function createExecTimeColumns(data) {
  return benchmarkNames.map(name => [
    name,
    ...data.map(d => {
      const benchmark = d.benchmark[name];
      const meanValue = benchmark ? benchmark.mean : 0;
      return meanValue || 0;
    })
  ]);
}

export function createBinarySizeColumns(data) {
  return [["binary_size", ...data.map(d => d.binary_size || 0)]];
}

export function createSha1List(data) {
  return data.map(d => d.sha1);
}

// Formats the byte sizes e.g. 19000 -> 18.55 KB
// Copied from https://stackoverflow.com/a/18650828
export function formatBytes(a, b) {
  if (0 == a) return "0 Bytes";
  var c = 1024,
    d = b || 2,
    e = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"],
    f = Math.floor(Math.log(a) / Math.log(c));
  return parseFloat((a / Math.pow(c, f)).toFixed(d)) + " " + e[f];
}

export async function main() {
  const data = await getJson("./data.json");

  const execTimeColumns = createExecTimeColumns(data);
  const binarySizeColumns = createBinarySizeColumns(data);
  const sha1List = createSha1List(data);

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
    data: { columns: binarySizeColumns },
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
}
