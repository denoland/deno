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

function formatRatio(report: { pass: number; total: number } | undefined) {
  if (!report) {
    return "N/A";
  }
  const ratio = (report.pass / report.total) * 100;
  return ratio.toFixed(2) + "%";
}

function createMessage(monthSummary: MonthSummary) {
  const daySummary = Object.values(monthSummary.reports).sort((a, b) =>
    new Date(a.date).getTime() - new Date(b.date).getTime()
  ).at(-1);
  if (!daySummary) {
    throw new Error("No summary data found");
  }
  const { date, linux, windows, darwin } = daySummary;
  const linuxRatio = formatRatio(linux);
  const windowsRatio = formatRatio(windows);
  const darwinRatio = formatRatio(darwin);
  const mrkdwn =
    `Linux *${linuxRatio}* / Windows *${windowsRatio}* / Darwin *${darwinRatio}* <https://node-test-viewer.deno.dev/results/${date}|(Full report)>`;

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
