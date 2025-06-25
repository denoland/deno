// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { LogLevel, WebClient } from "npm:@slack/web-api@7.8.0";
import type { MonthSummary } from "./add_day_summary_to_month_summary.ts";

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
  } catch (error) {
    console.error(error);
  }
}

await main();
