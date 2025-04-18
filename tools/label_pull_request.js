// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console camelcase

async function createComment(context, github) {
  await github.rest.issues.createComment({
    issue_number: context.issue.number,
    owner: context.repo.owner,
    repo: context.repo.repo,
    body: `Thanks for the PR!

Once you are done, please request a review from a Deno team member.`,
  });
}

async function updateLabels(context, github) {
  const result = await github.rest.issues
    .listLabelsOnIssue({
      issue_number: context.payload.pull_request.number,
      owner: context.repo.owner,
      repo: context.repo.repo,
    });
  const labelNames = result.data.map((label) => label.name);
  labelNames.push("pr:needs-review");

  return github.rest.issues.setLabels({
    issue_number: context.payload.pull_request.number,
    owner: context.repo.owner,
    repo: context.repo.repo,
    labels: labelNames,
  });
}

// TODO(bartlomieju): figure out how to use ES modules in GH Actions scripts
module.exports = async ({ context, github }) => {
  const eventHandlers = {
    opened: createComment,
    reopened: createComment,
    review_requested: updateLabels,
  };

  const eventName = context.payload.action;
  const eventHandler = eventHandlers[eventName];

  if (!eventHandler) {
    console.log(`::warning::'${eventName}' event has no handler, skipping.`);
    return;
  }

  try {
    await eventHandler(context, github);
  } catch (error) {
    console.log("::warning::Error, during update, bailing out: ", error);
  }
};
