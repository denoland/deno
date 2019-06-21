// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// How much to multiply time values in order to process log graphs properly.
const TimeScaleFactor = 10000;

export async function getJson(path) {
  return (await fetch(path)).json();
}

function getBenchmarkVarieties(data, benchmarkName) {
  // Look at last sha hash.
  const last = data[data.length - 1];
  return Object.keys(last[benchmarkName]);
}

export function createColumns(data, benchmarkName) {
  const varieties = getBenchmarkVarieties(data, benchmarkName);
  return varieties.map(variety => [
    variety,
    ...data.map(d => {
      if (d[benchmarkName] != null) {
        if (d[benchmarkName][variety] != null) {
          const v = d[benchmarkName][variety];
          if (benchmarkName == "benchmark") {
            const meanValue = v ? v.mean : 0;
            return meanValue || null;
          } else {
            return v;
          }
        }
      }
      return null;
    })
  ]);
}

export function createExecTimeColumns(data) {
  return createColumns(data, "benchmark");
}

export function createThroughputColumns(data) {
  return createColumns(data, "throughput");
}

export function createProxyColumns(data) {
  return createColumns(data, "req_per_sec_proxy");
}

export function createReqPerSecColumns(data) {
  return createColumns(data, "req_per_sec");
}

export function createMaxLatencyColumns(data) {
  return createColumns(data, "max_latency");
}

export function createMaxMemoryColumns(data) {
  return createColumns(data, "max_memory");
}

export function createBinarySizeColumns(data) {
  const propName = "binary_size";
  const binarySizeNames = Object.keys(data[data.length - 1][propName]);
  return binarySizeNames.map(name => [
    name,
    ...data.map(d => {
      const binarySizeData = d["binary_size"];
      switch (typeof binarySizeData) {
        case "number": // legacy implementation
          return name === "deno" ? binarySizeData : 0;
        default:
          if (!binarySizeData) {
            return null;
          }
          return binarySizeData[name] || null;
      }
    })
  ]);
}

export function createThreadCountColumns(data) {
  const propName = "thread_count";
  const threadCountNames = Object.keys(data[data.length - 1][propName]);
  return threadCountNames.map(name => [
    name,
    ...data.map(d => {
      const threadCountData = d[propName];
      if (!threadCountData) {
        return null;
      }
      return threadCountData[name] || null;
    })
  ]);
}

export function createSyscallCountColumns(data) {
  const propName = "syscall_count";
  const syscallCountNames = Object.keys(data[data.length - 1][propName]);
  return syscallCountNames.map(name => [
    name,
    ...data.map(d => {
      const syscallCountData = d[propName];
      if (!syscallCountData) {
        return null;
      }
      return syscallCountData[name] || null;
    })
  ]);
}

export function createSha1List(data) {
  return data.map(d => d.sha1);
}

export function formatMB(bytes) {
  return (bytes / (1024 * 1024)).toFixed(2);
}

export function formatReqSec(reqPerSec) {
  return reqPerSec / 1000;
}

/**
 * @param {string} id The id of dom element
 * @param {string[]} categories categories for x-axis values
 * @param {any[][]} columns The columns data
 * @param {function} onclick action on clicking nodes of chart
 * @param {string} yLabel label of y axis
 * @param {function} yTickFormat formatter of y axis ticks
 */
function generate(
  id,
  categories,
  columns,
  onclick,
  yLabel = "",
  yTickFormat = null
) {
  const yAxis = {
    padding: { bottom: 0 },
    min: 0,
    label: yLabel,
    tick: null
  };
  if (yTickFormat) {
    yAxis.tick = {
      format: yTickFormat
    };
    if (yTickFormat == logScale) {
      delete yAxis.min;
      for (let col of columns) {
        for (let i = 1; i < col.length; i++) {
          if (col[i] == null || col[i] === 0) {
            continue;
          }
          col[i] = Math.log10(col[i] * TimeScaleFactor);
        }
      }
    }
  }

  // @ts-ignore
  c3.generate({
    bindto: id,
    data: {
      columns,
      onclick
    },
    axis: {
      x: {
        type: "category",
        show: false,
        categories
      },
      y: yAxis
    }
  });
}

function logScale(t) {
  return (Math.pow(10, t) / TimeScaleFactor).toFixed(4);
}

function formatSecsAsMins(t) {
  // TODO use d3.round()
  const a = t % 60;
  const min = Math.floor(t / 60);
  return a < 30 ? min : min + 1;
}

/**
 * @param dataUrl The url of benchmark data json.
 */
export function drawCharts(dataUrl) {
  // TODO Using window["location"]["hostname"] instead of
  // window.location.hostname because when deno runs app_test.js it gets a type
  // error here, not knowing about window.location.  Ideally Deno would skip
  // type check entirely on JS files.
  if (window["location"]["hostname"] != "deno.github.io") {
    dataUrl = "https://denoland.github.io/deno/" + dataUrl;
  }
  return drawChartsFromBenchmarkData(dataUrl);
}

const proxyFields = [
  "req_per_sec"
  //"max_latency"
];
function extractProxyFields(data) {
  for (const row of data) {
    for (const field of proxyFields) {
      const d = row[field];
      if (!d) continue;
      const name = field + "_proxy";
      const newField = {};
      row[name] = newField;
      for (const k of Object.getOwnPropertyNames(d)) {
        if (k.includes("_proxy")) {
          const v = d[k];
          delete d[k];
          newField[k] = v;
        }
      }
    }
  }
}
/**
 * Draws the charts from the benchmark data stored in gh-pages branch.
 */
export async function drawChartsFromBenchmarkData(dataUrl) {
  const data = await getJson(dataUrl);

  // hack to extract proxy fields from req/s fields
  extractProxyFields(data);

  const execTimeColumns = createExecTimeColumns(data);
  const throughputColumns = createThroughputColumns(data);
  const reqPerSecColumns = createReqPerSecColumns(data);
  const proxyColumns = createProxyColumns(data);
  const maxLatencyColumns = createMaxLatencyColumns(data);
  const maxMemoryColumns = createMaxMemoryColumns(data);
  const binarySizeColumns = createBinarySizeColumns(data);
  const threadCountColumns = createThreadCountColumns(data);
  const syscallCountColumns = createSyscallCountColumns(data);
  const sha1List = createSha1List(data);
  const sha1ShortList = sha1List.map(sha1 => sha1.substring(0, 6));

  const viewCommitOnClick = _sha1List => d => {
    // @ts-ignore
    window.open(
      `https://github.com/denoland/deno/commit/${_sha1List[d["index"]]}`
    );
  };

  function gen(id, columns, yLabel = "", yTickFormat = null) {
    generate(
      id,
      sha1ShortList,
      columns,
      viewCommitOnClick(sha1List),
      yLabel,
      yTickFormat
    );
  }

  gen("#exec-time-chart", execTimeColumns, "seconds", logScale);
  gen("#throughput-chart", throughputColumns, "seconds", logScale);
  gen("#req-per-sec-chart", reqPerSecColumns, "1000 req/sec", formatReqSec);
  gen("#proxy-req-per-sec-chart", proxyColumns, "req/sec");
  gen("#max-latency-chart", maxLatencyColumns, "milliseconds", logScale);
  gen("#max-memory-chart", maxMemoryColumns, "megabytes", formatMB);
  gen("#binary-size-chart", binarySizeColumns, "megabytes", formatMB);
  gen("#thread-count-chart", threadCountColumns, "threads");
  gen("#syscall-count-chart", syscallCountColumns, "syscalls");
}

export function main(): void {
  window["chartWidth"] = 800;
  const overlay = window["document"].getElementById("spinner-overlay");

  function showSpinner() {
    overlay.style.display = "block";
  }

  function hideSpinner() {
    overlay.style.display = "none";
  }

  function updateCharts() {
    const u = window.location.hash.match("all") ? "./data.json" : "recent.json";

    showSpinner();

    drawCharts(u)
      .then(hideSpinner)
      .catch(hideSpinner);
  }
  updateCharts();

  window["onhashchange"] = updateCharts;
}
