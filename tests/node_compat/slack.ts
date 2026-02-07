// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { LogLevel, WebClient } from "npm:@slack/web-api@7.8.0";
import type { MonthSummary } from "./add_day_summary_to_month_summary.ts";
import { toJson } from "@std/streams/to-json";

const token = Deno.env.get("SLACK_TOKEN");
const channel = Deno.env.get("SLACK_CHANNEL");

if (!token) {
  console.error("SLACK_TOKEN is required");
}
if (!channel) {
  console.error("SLACK_CHANNEL is required");
}

const client = new WebClient(token, {
  logLevel: LogLevel.DEBUG,
});

type OSName = "linux" | "windows" | "darwin";
const OS_NAMES: OSName[] = ["linux", "windows", "darwin"];

/** Minimal type for the full report with per-test results. */
interface FullReport {
  date: string;
  // deno-lint-ignore no-explicit-any
  results: Record<string, [boolean | string, ...any[]]>;
}

function getRatio(report: { pass: number; total: number } | undefined) {
  if (!report) {
    return -1;
  }

  return (report.pass / report.total) * 100;
}

function formatRatio(ratio: number) {
  if (ratio === -1) {
    return "N/A";
  }
  return ratio.toFixed(2) + "%";
}

function formatDiff(diff: number) {
  if (diff === 0) {
    return `±0% 🟨`;
  }

  const diffStr = diff.toFixed(2);

  if (diff > 0) {
    return `+${diffStr}% 🟩`;
  } else {
    return `${diffStr}% 🟥`;
  }
}

function createMessage(monthSummary: MonthSummary) {
  const sortedMonthSummary = Object.values(monthSummary.reports).sort((a, b) =>
    new Date(a.date).getTime() - new Date(b.date).getTime()
  );

  const daySummary = sortedMonthSummary.at(-1);
  if (!daySummary) {
    throw new Error("No summary data found");
  }
  const prevDaySummary = sortedMonthSummary.at(-2);
  const { date, linux, windows, darwin } = daySummary;
  let mrkdwn = "";

  const currentLinuxRatio = getRatio(linux);
  const prevLinuxRatio = getRatio(prevDaySummary?.linux);
  const linuxRatioDiff = prevLinuxRatio !== -1
    ? currentLinuxRatio - prevLinuxRatio
    : 0;
  mrkdwn += `Linux *${formatRatio(currentLinuxRatio)}* (${
    formatDiff(linuxRatioDiff)
  })\n`;

  const currentWindowsRatio = getRatio(windows);
  const prevWindowsRatio = getRatio(prevDaySummary?.windows);
  const windowsRatioDiff = prevWindowsRatio !== -1
    ? currentWindowsRatio - prevWindowsRatio
    : 0;
  mrkdwn += `Windows *${formatRatio(currentWindowsRatio)}* (${
    formatDiff(windowsRatioDiff)
  })\n`;

  const currentDarwinRatio = getRatio(darwin);
  const prevDarwinRatio = getRatio(prevDaySummary?.darwin);
  const darwinRatioDiff = prevDarwinRatio !== -1
    ? currentDarwinRatio - prevDarwinRatio
    : 0;
  mrkdwn += `Darwin *${formatRatio(currentDarwinRatio)}* (${
    formatDiff(darwinRatioDiff)
  })\n`;

  mrkdwn += `<https://node-test-viewer.deno.dev/results/${date}|(Full report)>`;

  return [
    {
      type: "section",
      text: {
        type: "mrkdwn",
        text: mrkdwn,
      },
    },
  ];
}

async function fetchFullReport(
  date: string,
  os: OSName,
): Promise<FullReport | undefined> {
  try {
    const res = await fetch(
      `https://dl.deno.land/node-compat-test/${date}/report-${os}.json.gz`,
    );
    if (res.status === 404) return undefined;
    return await toJson(
      res.body!.pipeThrough(new DecompressionStream("gzip")),
    ) as FullReport;
  } catch (e) {
    console.error(`Failed to fetch report for ${date}/${os}:`, e);
    return undefined;
  }
}

type TestStatus = "pass" | "fail" | "ignore" | "missing";

function getTestStatus(
  report: FullReport | undefined,
  testName: string,
): TestStatus {
  if (!report) return "missing";
  const result = report.results[testName];
  if (!result) return "missing";
  if (result[0] === true) return "pass";
  if (result[0] === false) return "fail";
  return "ignore";
}

function getLastNDates(referenceDate: string, n: number): string[] {
  const dates: string[] = [];
  const d = new Date(referenceDate);
  for (let i = 1; i <= n; i++) {
    const prev = new Date(d);
    prev.setDate(d.getDate() - i);
    dates.push(prev.toISOString().slice(0, 10));
  }
  return dates;
}

async function fetchReportsForDate(
  date: string,
): Promise<Record<OSName, FullReport | undefined>> {
  const entries = await Promise.all(
    OS_NAMES.map(
      async (os) => [os, await fetchFullReport(date, os)] as const,
    ),
  );
  return Object.fromEntries(entries) as Record<OSName, FullReport | undefined>;
}

function formatOsList(oses: OSName[]): string {
  return oses.length === OS_NAMES.length ? "all" : oses.join(", ");
}

// deno-lint-ignore no-explicit-any
type Block = { type: string; text: { type: string; text: string } } | any;

const MAX_NEWLY_PASSING = 30;
const MAX_NEWLY_FAILING = 30;
const MAX_FLAKY = 20;
const FLAKY_HISTORY_DAYS = 14; // + today = 15 runs

async function generateThreadBlocks(
  monthSummary: MonthSummary,
): Promise<Block[] | null> {
  const sortedReports = Object.values(monthSummary.reports).sort(
    (a, b) => new Date(a.date).getTime() - new Date(b.date).getTime(),
  );
  const todaySummary = sortedReports.at(-1);
  if (!todaySummary) return null;
  const prevSummary = sortedReports.at(-2);

  const todayDate = todaySummary.date;
  console.log("Fetching full reports for thread insights...");

  // Fetch today's full reports
  const todayReports = await fetchReportsForDate(todayDate);

  // Fetch previous run's reports (if available)
  let prevReports: Record<OSName, FullReport | undefined> | undefined;
  if (prevSummary) {
    prevReports = await fetchReportsForDate(prevSummary.date);
  }

  // Collect all test names from today + previous
  const allTestNames = new Set<string>();
  for (const os of OS_NAMES) {
    for (const name of Object.keys(todayReports[os]?.results ?? {})) {
      allTestNames.add(name);
    }
    if (prevReports) {
      for (const name of Object.keys(prevReports[os]?.results ?? {})) {
        allTestNames.add(name);
      }
    }
  }

  // Find newly passing and newly failing tests
  const newlyPassing = new Map<string, OSName[]>();
  const newlyFailing = new Map<string, OSName[]>();

  if (prevReports) {
    for (const testName of allTestNames) {
      for (const os of OS_NAMES) {
        const prevStatus = getTestStatus(prevReports[os], testName);
        const todayStatus = getTestStatus(todayReports[os], testName);

        if (prevStatus === "fail" && todayStatus === "pass") {
          if (!newlyPassing.has(testName)) newlyPassing.set(testName, []);
          newlyPassing.get(testName)!.push(os);
        } else if (prevStatus === "pass" && todayStatus === "fail") {
          if (!newlyFailing.has(testName)) newlyFailing.set(testName, []);
          newlyFailing.get(testName)!.push(os);
        }
      }
    }
  }

  // Fetch historical reports for flaky detection
  console.log("Fetching historical reports for flaky detection...");
  const historicalDates = getLastNDates(todayDate, FLAKY_HISTORY_DAYS);
  const allReports = new Map<string, Record<OSName, FullReport | undefined>>();
  allReports.set(todayDate, todayReports);

  const historicalEntries = await Promise.all(
    historicalDates.map(async (date) => {
      const reports = await fetchReportsForDate(date);
      return [date, reports] as const;
    }),
  );
  for (const [date, reports] of historicalEntries) {
    allReports.set(date, reports);
  }

  // Detect flaky tests: tests that both passed and failed at least 2 times
  const flakyTests = new Map<
    string,
    Map<OSName, { pass: number; total: number }>
  >();

  const allHistoricalTestNames = new Set<string>();
  for (const reports of allReports.values()) {
    for (const os of OS_NAMES) {
      for (const name of Object.keys(reports[os]?.results ?? {})) {
        allHistoricalTestNames.add(name);
      }
    }
  }

  for (const testName of allHistoricalTestNames) {
    for (const os of OS_NAMES) {
      let passCount = 0;
      let failCount = 0;

      for (const reports of allReports.values()) {
        const status = getTestStatus(reports[os], testName);
        if (status === "pass") passCount++;
        else if (status === "fail") failCount++;
      }

      if (passCount >= 2 && failCount >= 2) {
        if (!flakyTests.has(testName)) {
          flakyTests.set(testName, new Map());
        }
        flakyTests.get(testName)!.set(os, {
          pass: passCount,
          total: passCount + failCount,
        });
      }
    }
  }

  // Build Slack blocks
  const blocks: Block[] = [];

  if (newlyPassing.size > 0) {
    const sorted = [...newlyPassing.entries()].sort(([a], [b]) =>
      a.localeCompare(b)
    );
    let text = `*Newly Passing (${sorted.length}):*\n`;
    for (const [testName, oses] of sorted.slice(0, MAX_NEWLY_PASSING)) {
      text += `\`${testName}\` (${formatOsList(oses)})\n`;
    }
    if (sorted.length > MAX_NEWLY_PASSING) {
      text += `_...and ${sorted.length - MAX_NEWLY_PASSING} more_\n`;
    }
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  if (newlyFailing.size > 0) {
    const sorted = [...newlyFailing.entries()].sort(([a], [b]) =>
      a.localeCompare(b)
    );
    let text = `*Started Failing (${sorted.length}):*\n`;
    for (const [testName, oses] of sorted.slice(0, MAX_NEWLY_FAILING)) {
      text += `\`${testName}\` (${formatOsList(oses)})\n`;
    }
    if (sorted.length > MAX_NEWLY_FAILING) {
      text += `_...and ${sorted.length - MAX_NEWLY_FAILING} more_\n`;
    }
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  if (flakyTests.size > 0) {
    const sorted = [...flakyTests.entries()].sort(([a], [b]) =>
      a.localeCompare(b)
    );
    let text = `*Flaky Tests (last ${
      FLAKY_HISTORY_DAYS + 1
    } runs, ${sorted.length} tests):*\n`;
    for (const [testName, osStats] of sorted.slice(0, MAX_FLAKY)) {
      const parts: string[] = [];
      for (const [os, stats] of osStats) {
        parts.push(`${os}: ${stats.pass}/${stats.total}`);
      }
      text += `\`${testName}\` ${parts.join(", ")}\n`;
    }
    if (sorted.length > MAX_FLAKY) {
      text += `_...and ${sorted.length - MAX_FLAKY} more_\n`;
    }
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  if (blocks.length === 0) {
    blocks.push({
      type: "section",
      text: { type: "mrkdwn", text: "No test status changes detected." },
    });
  }

  return blocks;
}

async function main() {
  const monthSummary = await Deno.readTextFile("tests/node_compat/summary.json")
    .then(JSON.parse) as MonthSummary;

  try {
    const result = await client.chat.postMessage({
      token,
      channel,
      blocks: createMessage(monthSummary),
      unfurl_links: false,
      unfurl_media: false,
    });

    console.log(result);

    // Post thread with detailed insights
    if (result.ok && result.ts) {
      try {
        const threadBlocks = await generateThreadBlocks(monthSummary);
        if (threadBlocks) {
          const threadResult = await client.chat.postMessage({
            token,
            channel,
            thread_ts: result.ts,
            blocks: threadBlocks,
            unfurl_links: false,
            unfurl_media: false,
          });
          console.log("Thread posted:", threadResult);
        }
      } catch (threadError) {
        console.error("Failed to post thread:", threadError);
      }
    }
  } catch (error) {
    console.error(error);
  }
}

await main();
