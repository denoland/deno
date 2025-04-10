// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { LogLevel, WebClient } from "npm:@slack/web-api@7.8.0";
import {
  fetchMonthSummary,
  type MonthSummary,
} from "./add_day_summary_to_month_summary.ts";

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
  const mrkdwn = `
*Results* _${date}_
Linux *${linuxRatio}*
Windows *${windowsRatio}*
Darwin *${darwinRatio}*
<https://node-test-viewer.deno.dev/results/${date}|View full report>
`;

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
  const monthSummary = await fetchMonthSummary(
    new Date().toISOString().slice(0, 7),
  );
  if (!monthSummary) {
    console.error("No month summary found");
    Deno.exit(1);
  }
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
