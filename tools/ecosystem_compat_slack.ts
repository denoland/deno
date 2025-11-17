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
  return (duration / 1000).toFixed(0) + "s";
}

function createMessage(ecosystemReports: Record<string, EcosystemReport>) {
  const elements = [];

  elements.push({
    type: "section",
    text: {
      type: "mrkdwn",
      text: "*Package manager report*\n\n",
    },
  });

  const tableHeader = [
    {
      type: "rich_text",
      elements: [
        {
          type: "rich_text_section",
          elements: [
            {
              type: "text",
              text: "Program",
              style: {
                bold: true,
              },
            },
          ],
        },
      ],
    },
    {
      type: "rich_text",
      elements: [
        {
          type: "rich_text_section",
          elements: [
            {
              type: "text",
              text: "Linux",
              style: {
                bold: true,
              },
            },
          ],
        },
      ],
    },
    {
      type: "rich_text",
      elements: [
        {
          type: "rich_text_section",
          elements: [
            {
              type: "text",
              text: "macOS",
              style: {
                bold: true,
              },
            },
          ],
        },
      ],
    },
    {
      type: "rich_text",
      elements: [
        {
          type: "rich_text_section",
          elements: [
            {
              type: "text",
              text: "Windows",
              style: {
                bold: true,
              },
            },
          ],
        },
      ],
    },
  ];

  const rows = [];

  const programs = Object.keys(ecosystemReports["darwin"]);
  for (const program of programs) {
    const row = [
      {
        type: "rich_text",
        elements: [
          {
            type: "rich_text_section",
            elements: [
              {
                type: "text",
                text: program,
              },
            ],
          },
        ],
      },
    ];

    for (const os of ["darwin", "linux", "windows"]) {
      const report = ecosystemReports[os][program] satisfies PmResult;

      const text = `${
        report.exitCode === 0 ? "✅" : "❌"
      } code: ${report.exitCode}, (${formatDuration(report.duration)})`;
      row.push({
        type: "rich_text",
        elements: [
          {
            type: "rich_text_section",
            elements: [
              {
                type: "text",
                text: text,
                style: {
                  code: true,
                },
              },
            ],
          },
        ],
      });
    }

    rows.push(row);
  }

  elements.push({
    type: "table",
    rows: [tableHeader, ...rows],
  });
  return elements;
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
