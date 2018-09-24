// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { test, assertEqual } from "../js/test_util.ts";
import {
  createBinarySizeColumns,
  createExecTimeColumns,
  createThreadCountColumns,
  createSha1List,
  formatBytes
} from "./app.js";

const regularData = [
  {
    created_at: "2018-01-01T01:00:00Z",
    sha1: "abcdef",
    binary_size: 100000000,
    benchmark: {
      hello: {
        mean: 0.05
      },
      relative_import: {
        mean: 0.06
      }
    },
    thread_count: {
      set_timeout: 4
    }
  },
  {
    created_at: "2018-01-02T01:00:00Z",
    sha1: "012345",
    binary_size: 110000000,
    benchmark: {
      hello: {
        mean: 0.055
      },
      relative_import: {
        mean: 0.065
      }
    },
    thread_count: {
      set_timeout: 5
    }
  }
];

const irregularData = [
  {
    created_at: "2018-01-01T01:00:00Z",
    sha1: "123",
    benchmark: {
      hello: {},
      relative_import: {}
    },
    thread_count: {}
  },
  {
    created_at: "2018-02-01T01:00:00Z",
    sha1: "456",
    benchmark: {}
  }
];

test(function createExecTimeColumnsRegularData() {
  const columns = createExecTimeColumns(regularData);
  assertEqual(columns, [
    ["hello", 0.05, 0.055],
    ["relative_import", 0.06, 0.065]
  ]);
});

test(function createExecTimeColumnsIrregularData() {
  const columns = createExecTimeColumns(irregularData);
  assertEqual(columns, [["hello", 0, 0], ["relative_import", 0, 0]]);
});

test(function createBinarySizeColumnsRegularData() {
  const columns = createBinarySizeColumns(regularData);
  assertEqual(columns, [["binary_size", 100000000, 110000000]]);
});

test(function createBinarySizeColumnsIrregularData() {
  const columns = createBinarySizeColumns(irregularData);
  assertEqual(columns, [["binary_size", 0, 0]]);
});

test(function createThreadCountColumnsRegularData() {
  const columns = createThreadCountColumns(regularData);
  assertEqual(columns, [["set_timeout", 4, 5]]);
});

test(function createThreadCountColumnsIrregularData() {
  const columns = createThreadCountColumns(irregularData);
  assertEqual(columns, [["set_timeout", 0, 0]]);
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
