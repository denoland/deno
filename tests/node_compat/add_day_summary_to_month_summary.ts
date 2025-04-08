// Copyright 2018-2025 the Deno authors. MIT license.
// This file mostly mirrors the utils in
// https://github.com/denoland/node_test_viewer/blob/a642f725f2d9595bc8cf217c41967c446814a79e/util/report.ts

// deno-lint-ignore-file no-console

import type {
  SingleResult,
  TestReportMetadata,
} from "./run_all_test_unmodified.ts";
import { toJson } from "@std/streams/to-json";

/** The test report format, which is stored in JSON file */
export type TestReport = TestReportMetadata & {
  results: Record<string, SingleResult>;
};

export type DayReport = {
  date: string;
  windows: TestReport | undefined;
  linux: TestReport | undefined;
  darwin: TestReport | undefined;
};

export type DaySummary = {
  date: string;
  windows: TestReportMetadata | undefined;
  linux: TestReportMetadata | undefined;
  darwin: TestReportMetadata | undefined;
};

export type MonthSummary = {
  reports: Record<string, DaySummary>;
  month: string;
};

export async function fetchMonthSummary(
  month: string,
): Promise<MonthSummary> {
  console.log("fetching", month);
  const res = await fetch(
    `https://dl.deno.land/node-compat-test/summary-${month}.json.gz`,
  );
  if (res.status === 404) {
    return { reports: {}, month };
  }
  try {
    const summary = await toJson(
      res.body!.pipeThrough(new DecompressionStream("gzip")),
    );
    return summary as MonthSummary;
  } catch (e) {
    console.error(e);
    return { reports: {}, month };
  }
}

/** Gets the report summary for the given date. */
export async function fetchDaySummary(date: string): Promise<DaySummary> {
  const windows = await fetchReport(date, "windows");
  const linux = await fetchReport(date, "linux");
  const darwin = await fetchReport(date, "darwin");
  return {
    date,
    windows: extractMetadata(windows),
    linux: extractMetadata(linux),
    darwin: extractMetadata(darwin),
  };
}

function extractMetadata(
  report: TestReport | undefined,
): TestReportMetadata | undefined {
  if (!report) {
    return undefined;
  }
  const { date, denoVersion, os, arch, nodeVersion, runId, total, pass } =
    report;
  return {
    date,
    denoVersion,
    os,
    arch,
    nodeVersion,
    runId,
    total,
    pass,
  };
}

export async function fetchReport(
  date: string,
  os: "linux" | "windows" | "darwin",
): Promise<TestReport | undefined> {
  console.log("fetching", date, os);
  try {
    const res = await fetch(
      `https://dl.deno.land/node-compat-test/${date}/report-${os}.json.gz`,
    );
    if (res.status === 404) {
      return undefined;
    }
    const report = await toJson(
      res.body!.pipeThrough(new DecompressionStream("gzip")),
    );
    return report as TestReport;
  } catch (e) {
    console.error(e);
    return undefined;
  }
}

async function main() {
  const date = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
  const month = date.slice(0, 7); // YYYY-MM

  const monthSummary = await fetchMonthSummary(month);
  const daySummary = await fetchDaySummary(date);
  monthSummary.reports[date] = daySummary;
  // sort the reports by date
  const reports = Object.entries(monthSummary.reports).sort(
    ([a], [b]) => new Date(a).getTime() - new Date(b).getTime(),
  );
  monthSummary.reports = Object.fromEntries(reports);
  console.log("Generated month summary:", monthSummary);
  const summaryPath = "tests/node_compat/summary.json";
  // Store the results in a JSON file
  console.log("Writing month summary to file", summaryPath);
  await Deno.writeTextFile(summaryPath, JSON.stringify(monthSummary));
}

if (import.meta.main) {
  await main();
}
