// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { LogLevel, WebClient } from "npm:@slack/web-api@7.8.0";

const token = Deno.env.get("SLACK_TOKEN");
const channel = Deno.env.get("SLACK_CHANNEL");

if (!token) {
  console.error("SLACK_TOKEN is required");
}
if (!channel) {
  console.error("SLACK_CHANNEL is required");
}

interface PmResult {
  exitCode: number;
  duration: number;
}

interface EcosystemReport {
  npm: PmResult;
  yarn: PmResult;
  pnpm: PmResult;
}

const client = new WebClient(token, {
  logLevel: LogLevel.DEBUG,
});

function formatDuration(duration: number) {
  return (duration / 1000).toFixed(2) + "s";
}

function createMessage(ecosystemReports: Record<string, EcosystemReport>) {
  let mrkdwn = "## Package manager report\n\n";

  mrkdwn += "| OS | npm | yarn | pnpm |\n";
  mrkdwn += "|----|-----|------|------|\n";
  for (const [os, report] of Object.entries(ecosystemReports)) {
    mrkdwn += `| ${os} | `;
    mrkdwn += `${
      report.npm.exitCode === 0 ? "✅" : "❌"
    } code: ${report.npm.exitCode}, duration: ${
      formatDuration(report.npm.duration)
    } | `;
    mrkdwn += `${
      report.yarn.exitCode === 0 ? "✅" : "❌"
    } code: ${report.yarn.exitCode}, duration: ${
      formatDuration(report.yarn.duration)
    } | `;
    mrkdwn += `${
      report.npm.exitCode === 0 ? "✅" : "❌"
    } code: ${report.pnpm.exitCode}, duration: ${
      formatDuration(report.pnpm.duration)
    } |\n`;
  }

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

async function downloadOsReports() {
  const oses = ["windows", "linux", "darwin"];
  const reports: Record<string, string> = {};
  for (const os of oses) {
    const res = await fetch(
      `https://dl.deno.land/ecosystem-compat-test/${
        new Date()
          .toISOString()
          .substring(0, 10)
      }/report-${os}.json`,
    );
    if (res.status === 200) {
      reports[os] = await res.json() satisfies EcosystemReport;
    }
  }
  return reports;
}

async function main() {
  const ecosystemReports = await downloadOsReports();

  try {
    const result = await client.chat.postMessage({
      token,
      channel,
      blocks: createMessage(ecosystemReports),
      unfurl_links: false,
      unfurl_media: false,
    });

    console.log(result);
  } catch (error) {
    console.error(error);
  }
}

await main();
