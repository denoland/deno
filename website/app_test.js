// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { test, assert, assertEqual } from "../js/test_util.ts";
import {
  createBinarySizeColumns,
  createExecTimeColumns,
  createThreadCountColumns,
  createSyscallCountColumns,
  createSha1List,
  formatBytes,
  formatSeconds,
  getTravisData
} from "./app.js";

const regularData = [
  {
    created_at: "2018-01-01T01:00:00Z",
    sha1: "abcdef",
    binary_size: {
      deno: 100000000,
      "main.js": 90000000,
      "main.js.map": 80000000,
      "snapshot_deno.bin": 70000000
    },
    benchmark: {
      hello: {
        mean: 0.05
      },
      relative_import: {
        mean: 0.06
      },
      cold_hello: {
        mean: 0.05
      },
      cold_relative_import: {
        mean: 0.06
      }
    },
    thread_count: {
      set_timeout: 4,
      fetch_deps: 6
    },
    syscall_count: {
      hello: 600
    }
  },
  {
    created_at: "2018-01-02T01:00:00Z",
    sha1: "012345",
    binary_size: {
      deno: 100000001,
      "main.js": 90000001,
      "main.js.map": 80000001,
      "snapshot_deno.bin": 70000001
    },
    benchmark: {
      hello: {
        mean: 0.055
      },
      relative_import: {
        mean: 0.065
      },
      cold_hello: {
        mean: 0.055
      },
      cold_relative_import: {
        mean: 0.065
      }
    },
    thread_count: {
      set_timeout: 5,
      fetch_deps: 7
    },
    syscall_count: {
      hello: 700
    }
  }
];

const irregularData = [
  {
    created_at: "2018-01-01T01:00:00Z",
    sha1: "123",
    binary_size: {},
    benchmark: {
      hello: {},
      relative_import: {},
      cold_hello: {},
      cold_relative_import: {}
    },
    thread_count: {},
    syscall_count: {}
  },
  {
    created_at: "2018-02-01T01:00:00Z",
    sha1: "456",
    binary_size: 100000000,
    benchmark: {}
  }
];

test(function createExecTimeColumnsRegularData() {
  const columns = createExecTimeColumns(regularData);
  assertEqual(columns, [
    ["hello", 0.05, 0.055],
    ["relative_import", 0.06, 0.065],
    ["cold_hello", 0.05, 0.055],
    ["cold_relative_import", 0.06, 0.065]
  ]);
});

test(function createExecTimeColumnsIrregularData() {
  const columns = createExecTimeColumns(irregularData);
  assertEqual(columns, [
    ["hello", 0, 0],
    ["relative_import", 0, 0],
    ["cold_hello", 0, 0],
    ["cold_relative_import", 0, 0]
  ]);
});

test(function createBinarySizeColumnsRegularData() {
  const columns = createBinarySizeColumns(regularData);
  assertEqual(columns, [
    ["deno", 100000000, 100000001],
    ["main.js", 90000000, 90000001],
    ["main.js.map", 80000000, 80000001],
    ["snapshot_deno.bin", 70000000, 70000001]
  ]);
});

test(function createBinarySizeColumnsIrregularData() {
  const columns = createBinarySizeColumns(irregularData);
  assertEqual(columns, [
    ["deno", 0, 100000000],
    ["main.js", 0, 0],
    ["main.js.map", 0, 0],
    ["snapshot_deno.bin", 0, 0]
  ]);
});

test(function createThreadCountColumnsRegularData() {
  const columns = createThreadCountColumns(regularData);
  assertEqual(columns, [["set_timeout", 4, 5], ["fetch_deps", 6, 7]]);
});

test(function createThreadCountColumnsIrregularData() {
  const columns = createThreadCountColumns(irregularData);
  assertEqual(columns, [["set_timeout", 0, 0], ["fetch_deps", 0, 0]]);
});

test(function createSyscallCountColumnsRegularData() {
  const columns = createSyscallCountColumns(regularData);
  assertEqual(columns, [["hello", 600, 700]]);
});

test(function createSyscallCountColumnsIrregularData() {
  const columns = createSyscallCountColumns(irregularData);
  assertEqual(columns, [["hello", 0, 0]]);
});

test(function createSha1ListRegularData() {
  const sha1List = createSha1List(regularData);
  assertEqual(sha1List, ["abcdef", "012345"]);
});

test(function formatBytesPatterns() {
  assertEqual(formatBytes(18000), "17.58 KB");
  assertEqual(formatBytes(1800000), "1.72 MB");
  assertEqual(formatBytes(180000000), "171.66 MB");
  assertEqual(formatBytes(18000000000), "16.76 GB");
});

test(function formatSecondsPatterns() {
  assertEqual(formatSeconds(10), "0 min");
  assertEqual(formatSeconds(100), "2 min");
  assertEqual(formatSeconds(1000), "17 min");
  assertEqual(formatSeconds(10000), "167 min");
});

test(async function getTravisDataSuccess() {
  try {
    const data = await getTravisData();
    assert(data.length !== 0);
  } catch (e) {
    assert(e !== null);
  }
});
