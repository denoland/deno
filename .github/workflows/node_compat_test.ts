#!/usr/bin/env -S deno run --check --allow-write=. --allow-read=. --lock=./tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
import {
  conditions,
  createWorkflow,
  defineMatrix,
  job,
  step,
} from "jsr:@david/gagen@0.3.1";

const isMainBranch = conditions.isBranch("main");

const matrix = defineMatrix({
  include: [
    { os: "linux", runner: "ubuntu-latest" },
    { os: "windows", runner: "windows-latest" },
    { os: "darwin", runner: "macos-latest" },
  ],
});

const checkout = step({
  name: "Checkout",
  uses: "actions/checkout@v6",
  with: { submodules: true },
});

const setupRust = step.dependsOn(checkout)({
  name: "Setup Rust",
  uses: "dsherret/rust-toolchain-file@v1",
});

const setupDeno = step.dependsOn(setupRust)({
  name: "Setup Deno",
  uses: "denoland/setup-deno@v2",
  with: { "deno-version": "canary" },
});

const installPython = step.dependsOn(setupDeno)({
  name: "Install Python",
  uses: "actions/setup-python@v6",
  with: { "python-version": 3.11 },
});

const authGcloud = step.dependsOn(installPython)({
  name: "Authenticate with Google Cloud",
  if: isMainBranch,
  uses: "google-github-actions/auth@v3",
  with: {
    project_id: "denoland",
    credentials_json: "${{ secrets.GCP_SA_KEY }}",
    export_environment_variables: true,
    create_credentials_file: true,
  },
});

const setupGcloud = step.dependsOn(authGcloud)({
  name: "Setup gcloud",
  if: isMainBranch,
  uses: "google-github-actions/setup-gcloud@v3",
  with: { project_id: "denoland" },
});

const runTests = step.dependsOn(setupGcloud)({
  name: "Run tests",
  env: {
    CARGO_ENCODED_RUSTFLAGS: "",
  },
  run: "deno task --cwd tests/node_compat/runner test --report",
});

const gzipReport = step.dependsOn(runTests)({
  name: "Gzip the report",
  run: "gzip tests/node_compat/report.json",
});

const uploadReport = step.dependsOn(gzipReport)({
  name: "Upload the report to dl.deno.land",
  if: isMainBranch,
  env: {
    AWS_ACCESS_KEY_ID: "${{ vars.S3_ACCESS_KEY_ID }}",
    AWS_SECRET_ACCESS_KEY: "${{ secrets.S3_SECRET_ACCESS_KEY }}",
    AWS_ENDPOINT_URL_S3: "${{ vars.S3_ENDPOINT }}",
    AWS_DEFAULT_REGION: "${{vars.S3_REGION }}",
  },
  run:
    "aws s3 cp tests/node_compat/report.json.gz s3://dl-deno-land/node-compat-test/$(date +%F)/report-${{matrix.os}}.json.gz",
});

const testJob = job("test", {
  runsOn: matrix.runner,
  strategy: {
    matrix,
  },
  steps: [
    checkout,
    setupRust,
    setupDeno,
    installPython,
    authGcloud,
    setupGcloud,
    runTests,
    gzipReport,
    uploadReport,
  ],
});

const summaryCheckout = step({
  name: "Checkout",
  uses: "actions/checkout@v6",
  with: { submodules: true },
});

const summarySetupDeno = step.dependsOn(summaryCheckout)({
  name: "Setup Deno",
  uses: "denoland/setup-deno@v2",
});

const summaryInstallPython = step.dependsOn(summarySetupDeno)({
  name: "Install Python",
  uses: "actions/setup-python@v6",
  with: { "python-version": 3.11 },
});

const summaryAuthGcloud = step.dependsOn(summaryInstallPython)({
  name: "Authenticate with Google Cloud",
  uses: "google-github-actions/auth@v3",
  with: {
    project_id: "denoland",
    credentials_json: "${{ secrets.GCP_SA_KEY }}",
    export_environment_variables: true,
    create_credentials_file: true,
  },
});

const summarySetupGcloud = step.dependsOn(summaryAuthGcloud)({
  name: "Setup gcloud",
  uses: "google-github-actions/setup-gcloud@v3",
  with: { project_id: "denoland" },
});

const addDaySummary = step.dependsOn(summarySetupGcloud)({
  name: "Add the day summary to the month summary",
  run:
    "deno -A --config tests/config/deno.json tests/node_compat/add_day_summary_to_month_summary.ts",
});

const gzipMonthSummary = step.dependsOn(addDaySummary)({
  name: "Gzip the month summary",
  run: "gzip tests/node_compat/summary.json -k",
});

const uploadMonthSummary = step.dependsOn(gzipMonthSummary)({
  name: "Upload the month summary",
  env: {
    AWS_ACCESS_KEY_ID: "${{ vars.S3_ACCESS_KEY_ID }}",
    AWS_SECRET_ACCESS_KEY: "${{ secrets.S3_SECRET_ACCESS_KEY }}",
    AWS_ENDPOINT_URL_S3: "${{ vars.S3_ENDPOINT }}",
    AWS_DEFAULT_REGION: "${{vars.S3_REGION }}",
  },
  run:
    "aws s3 cp tests/node_compat/summary.json.gz s3://dl-deno-land/node-compat-test/summary-$(date +%Y-%m).json.gz",
});

const postSlack = step.dependsOn(uploadMonthSummary)({
  name: "Post message to slack channel",
  run: "deno -A --config tests/config/deno.json tests/node_compat/slack.ts",
  env: {
    SLACK_TOKEN: "${{ secrets.NODE_COMPAT_SLACK_TOKEN }}",
    SLACK_CHANNEL: "${{ secrets.NODE_COMPAT_SLACK_CHANNEL }}",
  },
});

const workflow = createWorkflow({
  name: "node_compat_test",
  on: {
    schedule: [{ cron: "0 10 * * 1-5" }],
    workflow_dispatch: {},
  },
  jobs: [
    testJob,
    {
      id: "summary",
      runsOn: "ubuntu-latest",
      needs: [testJob],
      if: conditions.status.always().and(isMainBranch),
      steps: [postSlack],
    },
  ],
});

const header = "# GENERATED BY ./node_compat_test.ts -- DO NOT DIRECTLY EDIT";

export function generate() {
  return workflow.toYamlString({ header });
}

export const NODE_COMPAT_TEST_YML_URL = new URL(
  "./node_compat_test.generated.yml",
  import.meta.url,
);

if (import.meta.main) {
  workflow.writeOrLint({ filePath: NODE_COMPAT_TEST_YML_URL, header });
}
