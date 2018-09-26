// Copyright 2018 the Deno authors. All rights reserved. MIT license.

export async function getJson(path) {
  return (await fetch(path)).json();
}

export function getTravisData() {
  const url = "https://api.travis-ci.com/repos/denoland/deno/builds?event_type=pull_request";
  return fetch(url, {
    headers: {
      "Accept": "application/vnd.travis-ci.2.1+json"
    }
  })
  .then(res => res.json())
  .then(data => data.builds.reverse());
}

const benchmarkNames = [
  "hello",
  "relative_import",
  "cold_hello",
  "cold_relative_import"
];
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

const threadCountNames = ["set_timeout", "fetch_deps"];
export function createThreadCountColumns(data) {
  return threadCountNames.map(name => [
    name,
    ...data.map(d => {
      const threadCountData = d["thread_count"];
      if (!threadCountData) {
        return 0;
      }
      return threadCountData[name] || 0;
    })
  ]);
}

const syscallCountNames = ["hello"];
export function createSyscallCountColumns(data) {
  return syscallCountNames.map(name => [
    name,
    ...data.map(d => {
      const syscallCountData = d["syscall_count"];
      if (!syscallCountData) {
        return 0;
      }
      return syscallCountData[name] || 0;
    })
  ]);
}

const travisCompileTimeNames = ["duration_time"]
function createTravisCompileTimeColumns(data) {
  const columnsData = travisCompileTimeNames.map(name => [
    name,
    ...data.map(d => d.duration || 0)
  ]);
  return columnsData;
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

export function formatSeconds(t) {
  if (t < 0) {
    return "";
  }

  if (t === 0) {
    return "0 min";
  }
  const min = Math.floor(t / 60);
  const sec = t - min * 60;
  return min === 0 ? `${sec} sec` : `${min} min ${sec} sec`;
}

export async function main() {
  const data = await getJson("./data.json");
  const travisData = await getTravisData();

  const execTimeColumns = createExecTimeColumns(data);
  const binarySizeColumns = createBinarySizeColumns(data);
  const threadCountColumns = createThreadCountColumns(data);
  const syscallCountColumns = createSyscallCountColumns(data);
  const travisCompileTimeColumns = createTravisCompileTimeColumns(travisData);
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

  c3.generate({
    bindto: "#thread-count-chart",
    data: { columns: threadCountColumns },
    axis: {
      x: {
        type: "category",
        categories: sha1List
      }
    }
  });

  c3.generate({
    bindto: "#syscall-count-chart",
    data: { columns: syscallCountColumns },
    axis: {
      x: {
        type: "category",
        categories: sha1List
      }
    }
  });

  c3.generate({
    bindto: "#travis-compile-time-chart",
    data: { columns: travisCompileTimeColumns },
    axis: {
      x: {
        type: "category",
        categories: travisData.map(d => d.pull_request_number)
      },
      y: {
        tick: {
          format: d => formatSeconds(d)
        }
      }
    }
  });
}
