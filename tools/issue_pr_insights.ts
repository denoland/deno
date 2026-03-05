// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console camelcase

import { LogLevel, WebClient } from "npm:@slack/web-api@7.8.0";

const HASHY_URL = "https://hashy.deno.deno.net";
const HASHY_KEY = "issue_pr_insights_last_run";
const GITHUB_API = "https://api.github.com";
const REPO_OWNER = "denoland";
const REPO_NAME = "deno";

function getEnvOrExit(name: string): string {
  const value = Deno.env.get(name);
  if (!value) {
    console.error(`${name} is required`);
    Deno.exit(1);
  }
  return value;
}

const token = getEnvOrExit("SLACK_TOKEN");
const channel = getEnvOrExit("SLACK_CHANNEL");
const githubToken = getEnvOrExit("GITHUB_TOKEN");
const hoursInput = Deno.env.get("HOURS_LOOKBACK");

const client = new WebClient(token, {
  logLevel: LogLevel.DEBUG,
});

const headers: Record<string, string> = {
  "Accept": "application/vnd.github+json",
  "Authorization": `Bearer ${githubToken}`,
};

interface GitHubItem {
  number: number;
  title: string;
  html_url: string;
  created_at: string;
  /** Comment count from the GitHub API. For issues this counts all
   * comments; for PRs this only counts issue-style comments and does
   * NOT include review comments. */
  comments: number;
  pull_request?: unknown;
}

async function getLastRunTimestamp(): Promise<string | null> {
  if (hoursInput) {
    const hours = parseInt(hoursInput, 10);
    if (isNaN(hours) || hours <= 0) {
      console.error(
        `Invalid HOURS_LOOKBACK value: "${hoursInput}". ` +
          "Must be a positive number.",
      );
      Deno.exit(1);
    }
    const since = new Date(Date.now() - hours * 60 * 60 * 1000);
    return since.toISOString();
  }

  try {
    const res = await fetch(`${HASHY_URL}/hashes/${HASHY_KEY}`, {
      signal: AbortSignal.timeout(5000),
    });
    if (res.ok) {
      const text = await res.text();
      if (text) {
        return text;
      }
    }
    return null;
  } catch {
    return null;
  }
}

async function saveLastRunTimestamp(timestamp: string): Promise<void> {
  try {
    await fetch(`${HASHY_URL}/hashes/${HASHY_KEY}`, {
      method: "PUT",
      body: timestamp,
      signal: AbortSignal.timeout(5000),
    });
    console.log(`Saved last run timestamp: ${timestamp}`);
  } catch {
    console.error("Failed to save last run timestamp");
  }
}

async function fetchGitHubItems(
  type: "issues" | "pulls",
  since: string,
): Promise<GitHubItem[]> {
  const items: GitHubItem[] = [];
  let page = 1;
  const perPage = 100;

  while (true) {
    const params = new URLSearchParams({
      state: "all",
      sort: "created",
      direction: "desc",
      per_page: String(perPage),
      page: String(page),
    });
    if (type === "issues") {
      params.set("since", since);
    }

    const url =
      `${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}/${type}?${params}`;
    const res = await fetch(url, { headers });

    if (!res.ok) {
      console.error(
        `GitHub API error (${type} page ${page}): ` +
          `${res.status} ${res.statusText}`,
      );
      break;
    }

    const data = await res.json() as GitHubItem[];
    if (data.length === 0) break;

    for (const item of data) {
      if (new Date(item.created_at) >= new Date(since)) {
        items.push(item);
      } else if (type === "pulls") {
        // For pulls (sorted by created desc), once we pass the since date, stop
        return items;
      }
    }

    if (data.length < perPage) break;
    page++;
  }

  return items;
}

async function hasReviewComments(prNumber: number): Promise<boolean> {
  const url = `${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}` +
    `/pulls/${prNumber}/reviews`;
  const res = await fetch(url, { headers });
  if (!res.ok) return false;
  const reviews = await res.json() as unknown[];
  return reviews.length > 0;
}

async function filterNoResponsePRs(
  prs: GitHubItem[],
): Promise<GitHubItem[]> {
  const results: GitHubItem[] = [];
  for (const pr of prs) {
    if (pr.comments > 0) continue;
    if (await hasReviewComments(pr.number)) continue;
    results.push(pr);
  }
  return results;
}

function formatItemList(items: GitHubItem[], max: number): string {
  if (items.length === 0) return "_None_\n";
  let text = "";
  for (const item of items.slice(0, max)) {
    text += `• <${item.html_url}|#${item.number}> ${item.title}\n`;
  }
  if (items.length > max) {
    text += `_...and ${items.length - max} more_\n`;
  }
  return text;
}

interface SectionBlock {
  type: "section";
  text: { type: "mrkdwn"; text: string };
}

interface DividerBlock {
  type: "divider";
}

type Block = SectionBlock | DividerBlock;

function createBlocks(
  sinceDate: string,
  newIssues: GitHubItem[],
  newPRs: GitHubItem[],
  noResponseIssues: GitHubItem[],
  noResponsePRs: GitHubItem[],
): Block[] {
  const sinceStr = new Date(sinceDate).toUTCString();
  const blocks: Block[] = [];

  blocks.push({
    type: "section",
    text: {
      type: "mrkdwn",
      text: `*📊 Daily Issue & PR Insights*\n_Since ${sinceStr}_`,
    },
  });

  blocks.push({ type: "divider" });

  blocks.push({
    type: "section",
    text: {
      type: "mrkdwn",
      text: `*New Issues:* ${newIssues.length}\n*New PRs:* ${newPRs.length}`,
    },
  });

  blocks.push({ type: "divider" });

  let noResponseText =
    `*Issues with no response (${noResponseIssues.length}):*\n`;
  noResponseText += formatItemList(noResponseIssues, 15);
  blocks.push({
    type: "section",
    text: { type: "mrkdwn", text: noResponseText },
  });

  let noResponsePRText = `*PRs with no response (${noResponsePRs.length}):*\n`;
  noResponsePRText += formatItemList(noResponsePRs, 15);
  blocks.push({
    type: "section",
    text: { type: "mrkdwn", text: noResponsePRText },
  });

  return blocks;
}

async function postErrorMessage(message: string): Promise<void> {
  await client.chat.postMessage({
    token,
    channel,
    blocks: [
      {
        type: "section",
        text: {
          type: "mrkdwn",
          text: `*⚠️ Issue & PR Insights Error*\n${message}`,
        },
      },
    ],
    unfurl_links: false,
    unfurl_media: false,
  });
}

async function main() {
  const now = new Date().toISOString();

  const sinceDate = await getLastRunTimestamp();
  if (!sinceDate) {
    console.error("Could not determine last run timestamp");
    await postErrorMessage(
      "Could not determine last run timestamp. " +
        "The hashy service may be down or " +
        "there is no stored last-run value.",
    );
    // Still save the current timestamp so next run has a reference point
    await saveLastRunTimestamp(now);
    return;
  }

  console.log(`Fetching issues and PRs since: ${sinceDate}`);

  // The /issues endpoint returns both issues and PRs. We filter PRs out
  // by checking for `pull_request` field absence.
  const [allIssueItems, allPRs] = await Promise.all([
    fetchGitHubItems("issues", sinceDate),
    fetchGitHubItems("pulls", sinceDate),
  ]);

  // Filter out pull requests from the issues endpoint results
  const newIssues = allIssueItems.filter((item) => !item.pull_request);
  const newPRs = allPRs;

  const noResponseIssues = newIssues.filter((i) => i.comments === 0);
  const noResponsePRs = await filterNoResponsePRs(newPRs);

  console.log(`New issues: ${newIssues.length}`);
  console.log(`New PRs: ${newPRs.length}`);
  console.log(`Issues with no response: ${noResponseIssues.length}`);
  console.log(`PRs with no response: ${noResponsePRs.length}`);

  const blocks = createBlocks(
    sinceDate,
    newIssues,
    newPRs,
    noResponseIssues,
    noResponsePRs,
  );

  try {
    const result = await client.chat.postMessage({
      token,
      channel,
      blocks,
      unfurl_links: false,
      unfurl_media: false,
    });
    console.log("Message posted:", result.ok);
  } catch (error) {
    console.error("Failed to post Slack message:", error);
  }

  // Save the current run timestamp (only when not using manual hours input)
  if (!hoursInput) {
    await saveLastRunTimestamp(now);
  }
}

await main();
