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
  state: "open" | "closed";
}

interface GitHubComment {
  created_at: string;
  user?: { login: string };
}

interface HotIssue {
  item: GitHubItem;
  recentComments: number;
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

async function fetchRecentCommentCount(
  issueNumber: number,
  since: string,
): Promise<number> {
  const url = `${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}` +
    `/issues/${issueNumber}/comments?since=${since}&per_page=100`;
  const res = await fetch(url, { headers });
  if (!res.ok) return 0;
  const comments = await res.json() as GitHubComment[];
  // Exclude bot comments
  return comments.filter((c) => c.user && !c.user.login.endsWith("[bot]"))
    .length;
}

async function findHotIssues(
  since: string,
  minComments = 3,
): Promise<HotIssue[]> {
  // Fetch issues updated since the last run (includes old issues with new comments)
  const params = new URLSearchParams({
    state: "open",
    sort: "updated",
    direction: "desc",
    since,
    per_page: "100",
  });
  const url = `${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}/issues?${params}`;
  const res = await fetch(url, { headers });
  if (!res.ok) return [];

  const items = await res.json() as GitHubItem[];
  // Only actual issues (not PRs), with enough total comments to be worth checking
  const candidates = items.filter(
    (item) => !item.pull_request && item.comments >= minComments,
  );

  const results: HotIssue[] = [];
  for (const item of candidates) {
    const recentComments = await fetchRecentCommentCount(item.number, since);
    if (recentComments >= minComments) {
      results.push({ item, recentComments });
    }
  }

  results.sort((a, b) => b.recentComments - a.recentComments);
  return results;
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

function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str;
  return str.slice(0, maxLen - 1) + "…";
}

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const hours = Math.floor(diff / (1000 * 60 * 60));
  if (hours < 1) return "<1h";
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  return `${days}d`;
}

function formatItemList(items: GitHubItem[], max: number): string {
  if (items.length === 0) return "_None_\n";
  let text = "";
  const maxTitleLen = 55;
  for (const item of items.slice(0, max)) {
    const age = timeAgo(item.created_at);
    const title = truncate(item.title, maxTitleLen);
    text += `• <${item.html_url}|#${item.number}> ${title}  _(${age})_\n`;
  }
  if (items.length > max) {
    text += `_...and ${items.length - max} more_\n`;
  }
  return text;
}

function formatHotIssueList(issues: HotIssue[], max: number): string {
  let text = "";
  const maxTitleLen = 45;
  for (const { item, recentComments } of issues.slice(0, max)) {
    const title = truncate(item.title, maxTitleLen);
    text +=
      `• <${item.html_url}|#${item.number}> ${title}  _(${recentComments} new comments)_\n`;
  }
  if (issues.length > max) {
    text += `_...and ${issues.length - max} more_\n`;
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

function formatSinceRange(sinceDate: string): string {
  const since = new Date(sinceDate);
  const now = new Date();
  const diffH = Math.round(
    (now.getTime() - since.getTime()) / (1000 * 60 * 60),
  );
  const fmtDate = (d: Date) =>
    d.toLocaleDateString("en-US", {
      weekday: "short",
      month: "short",
      day: "numeric",
    });
  if (diffH < 24) return `last ${diffH}h`;
  return `${fmtDate(since)} → ${fmtDate(now)}`;
}

interface InsightsData {
  sinceDate: string;
  newIssues: GitHubItem[];
  newPRs: GitHubItem[];
  noResponseIssues: GitHubItem[];
  readyPRs: GitHubItem[];
  hotIssues: HotIssue[];
}

function totalItems(data: InsightsData): number {
  return data.hotIssues.length + data.noResponseIssues.length +
    data.readyPRs.length;
}

const MAX_ITEMS_MAIN = 20;
const MAX_ITEMS_PER_LIST = 25;

function buildSectionBlocks(data: InsightsData, max: number): Block[] {
  const blocks: Block[] = [];

  if (data.hotIssues.length > 0) {
    blocks.push({ type: "divider" });
    let text = `*:fire: Hot issues (${data.hotIssues.length}):*\n`;
    text += formatHotIssueList(data.hotIssues, max);
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  if (data.noResponseIssues.length > 0) {
    blocks.push({ type: "divider" });
    let text =
      `*:speech_balloon: Issues needing response (${data.noResponseIssues.length}):*\n`;
    text += formatItemList(data.noResponseIssues, max);
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  if (data.readyPRs.length > 0) {
    blocks.push({ type: "divider" });
    let text = `*:eyes: PRs needing review (${data.readyPRs.length}):*\n`;
    text += formatItemList(data.readyPRs, max);
    blocks.push({ type: "section", text: { type: "mrkdwn", text } });
  }

  return blocks;
}

function createHeaderBlock(data: InsightsData): Block {
  const range = formatSinceRange(data.sinceDate);
  return {
    type: "section",
    text: {
      type: "mrkdwn",
      text: `*:bar_chart: Issue & PR Insights* (${range})\n` +
        `>${data.newIssues.length} new issues, ${data.newPRs.length} new PRs`,
    },
  };
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
  // Only count open PRs and exclude WIP PRs
  const newPRs = allPRs.filter((pr) =>
    pr.state === "open" && !/^\[?wip\]?/i.test(pr.title.trim())
  );

  const noResponseIssues = newIssues.filter((i) => i.comments === 0);
  const [allNoResponsePRs, hotIssues] = await Promise.all([
    filterNoResponsePRs(newPRs),
    findHotIssues(sinceDate),
  ]);

  // Exclude WIP PRs entirely - they're not ready for review
  const readyPRs = allNoResponsePRs.filter(
    (pr) => !/^\[?wip\]?/i.test(pr.title.trim()),
  );

  console.log(`New issues: ${newIssues.length}`);
  console.log(`New PRs: ${newPRs.length}`);
  console.log(`Issues with no response: ${noResponseIssues.length}`);
  console.log(`PRs with no response: ${allNoResponsePRs.length}`);
  console.log(`Hot issues: ${hotIssues.length}`);

  const data: InsightsData = {
    sinceDate,
    newIssues,
    newPRs,
    noResponseIssues,
    readyPRs,
    hotIssues,
  };

  const needsThread = totalItems(data) > MAX_ITEMS_MAIN;

  try {
    // Main message: header + compact lists (or full lists if small enough)
    const mainBlocks: Block[] = [createHeaderBlock(data)];
    if (needsThread) {
      // Compact main message — show only counts, details in thread
      const summary: string[] = [];
      if (hotIssues.length > 0) {
        summary.push(`:fire: ${hotIssues.length} hot issues`);
      }
      if (noResponseIssues.length > 0) {
        summary.push(
          `:speech_balloon: ${noResponseIssues.length} issues need response`,
        );
      }
      if (readyPRs.length > 0) {
        summary.push(`:eyes: ${readyPRs.length} PRs need review`);
      }
      mainBlocks.push({ type: "divider" });
      mainBlocks.push({
        type: "section",
        text: {
          type: "mrkdwn",
          text: summary.join("\n") + "\n_See thread for details :thread:_",
        },
      });
    } else {
      mainBlocks.push(...buildSectionBlocks(data, MAX_ITEMS_PER_LIST));
    }

    const result = await client.chat.postMessage({
      token,
      channel,
      blocks: mainBlocks,
      unfurl_links: false,
      unfurl_media: false,
    });
    console.log("Message posted:", result.ok);

    // Post detailed thread reply if needed
    if (needsThread && result.ok && result.ts) {
      const threadBlocks = buildSectionBlocks(data, MAX_ITEMS_PER_LIST);
      await client.chat.postMessage({
        token,
        channel,
        thread_ts: result.ts,
        blocks: threadBlocks,
        unfurl_links: false,
        unfurl_media: false,
      });
      console.log("Thread reply posted");
    }
  } catch (error) {
    console.error("Failed to post Slack message:", error);
  }

  // Save the current run timestamp (only when not using manual hours input)
  if (!hoursInput) {
    await saveLastRunTimestamp(now);
  }
}

await main();
