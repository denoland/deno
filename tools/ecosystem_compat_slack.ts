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

function createMessage(ecosystemReport: EcosystemReport) {
  let mrkdwn = "Package manager report\n";

  mrkdwn += `*npm*: exit code: ${ecosystemReport.npm.exitCode}, duration: ${
    formatDuration(ecosystemReport.npm.duration)
  }\n`;
  mrkdwn += `*yarn*: exit code: ${ecosystemReport.yarn.exitCode}, duration: ${
    formatDuration(ecosystemReport.yarn.duration)
  }\n`;
  mrkdwn += `*pnpm*: exit code: ${ecosystemReport.pnpm.exitCode}, duration: ${
    formatDuration(ecosystemReport.pnpm.duration)
  }\n`;

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
  const ecosystemReport = await Deno.readTextFile(
    import.meta.resolve("./ecosystem_report.json"),
  )
    .then(JSON.parse) as EcosystemReport;

  try {
    const result = await client.chat.postMessage({
      token,
      channel,
      blocks: createMessage(ecosystemReport),
      unfurl_links: false,
      unfurl_media: false,
    });

    console.log(result);
  } catch (error) {
    console.error(error);
  }
}

await main();
