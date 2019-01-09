// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { test, testPerm, assert, assertEqual } from "../js/test_util.ts";
import {
  createBinarySizeColumns,
  createExecTimeColumns,
  createThreadCountColumns,
  createSyscallCountColumns,
  createSha1List,
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
    throughput: {
      "100M_tcp": 3.6,
      "100M_cat": 3.0,
      "10M_tcp": 1.6,
      "10M_cat": 1.0
    },
    req_per_sec: {
      node: 16000,
      deno: 1000
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
      hello: 600,
      fetch_deps: 700
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
    throughput: {
      "100M_tcp": 3.6,
      "100M_cat": 3.0,
      "10M_tcp": 1.6,
      "10M_cat": 1.0
    },
    req_per_sec: {
      node: 1600,
      deno: 3.0
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
      hello: 700,
      fetch_deps: 800
    }
  }
];

const irregularData = [
  {
    created_at: "2018-01-01T01:00:00Z",
    sha1: "123",
    benchmark: {},
    throughput: {},
    binary_size: {},
    thread_count: {},
    syscall_count: {}
  },
  {
    created_at: "2018-02-01T01:00:00Z",
    sha1: "456",
    benchmark: {
      hello: {},
      relative_import: {},
      cold_hello: {},
      cold_relative_import: {}
    },
    throughput: {
      "100M_tcp": 3.0
    },
    binary_size: {
      deno: 1
    },
    thread_count: {
      set_timeout: 5,
      fetch_deps: 7
    },
    syscall_count: {
      hello: 700,
      fetch_deps: 800
    }
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
    ["hello", null, null],
    ["relative_import", null, null],
    ["cold_hello", null, null],
    ["cold_relative_import", null, null]
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
  assertEqual(columns, [["deno", null, 1]]);
});

test(function createThreadCountColumnsRegularData() {
  const columns = createThreadCountColumns(regularData);
  assertEqual(columns, [["set_timeout", 4, 5], ["fetch_deps", 6, 7]]);
});

test(function createThreadCountColumnsIrregularData() {
  const columns = createThreadCountColumns(irregularData);
  assertEqual(columns, [["set_timeout", null, 5], ["fetch_deps", null, 7]]);
});

test(function createSyscallCountColumnsRegularData() {
  const columns = createSyscallCountColumns(regularData);
  assertEqual(columns, [["hello", 600, 700], ["fetch_deps", 700, 800]]);
});

test(function createSyscallCountColumnsIrregularData() {
  const columns = createSyscallCountColumns(irregularData);
  assertEqual(columns, [["hello", null, 700], ["fetch_deps", null, 800]]);
});

test(function createSha1ListRegularData() {
  const sha1List = createSha1List(regularData);
  assertEqual(sha1List, ["abcdef", "012345"]);
});

testPerm({ net: true }, async function getTravisDataSuccess() {
  const data = await getTravisData(
    "http://localhost:4545/tools/testdata/travis_benchmark.json"
  );
  assert(data.length !== 0);
});
