import { paths } from "/-/@octokit/openapi-types@v11.2.0-gu1h7aCgAJhI0Ck7YfZg/dist=es2019,mode=types/index.d.ts";
import { OctokitResponse } from "../OctokitResponse.d.ts";
import { RequestHeaders } from "../RequestHeaders.d.ts";
import { RequestRequestOptions } from "../RequestRequestOptions.d.ts";
declare type UnionToIntersection<U> = (U extends any ? (k: U) => void : never) extends (k: infer I) => void ? I : never;
declare type ExtractParameters<T> = "parameters" extends keyof T ? UnionToIntersection<{
    [K in keyof T["parameters"]]: T["parameters"][K];
}[keyof T["parameters"]]> : {};
declare type ExtractRequestBody<T> = "requestBody" extends keyof T ? "content" extends keyof T["requestBody"] ? "application/json" extends keyof T["requestBody"]["content"] ? T["requestBody"]["content"]["application/json"] : {
    data: {
        [K in keyof T["requestBody"]["content"]]: T["requestBody"]["content"][K];
    }[keyof T["requestBody"]["content"]];
} : "application/json" extends keyof T["requestBody"] ? T["requestBody"]["application/json"] : {
    data: {
        [K in keyof T["requestBody"]]: T["requestBody"][K];
    }[keyof T["requestBody"]];
} : {};
declare type ToOctokitParameters<T> = ExtractParameters<T> & ExtractRequestBody<T>;
declare type RequiredPreview<T> = T extends string ? {
    mediaType: {
        previews: [T, ...string[]];
    };
} : {};
declare type Operation<Url extends keyof paths, Method extends keyof paths[Url], preview = unknown> = {
    parameters: ToOctokitParameters<paths[Url][Method]> & RequiredPreview<preview>;
    request: {
        method: Method extends keyof MethodsMap ? MethodsMap[Method] : never;
        url: Url;
        headers: RequestHeaders;
        request: RequestRequestOptions;
    };
    response: ExtractOctokitResponse<paths[Url][Method]>;
};
declare type MethodsMap = {
    delete: "DELETE";
    get: "GET";
    patch: "PATCH";
    post: "POST";
    put: "PUT";
};
declare type SuccessStatuses = 200 | 201 | 202 | 204;
declare type RedirectStatuses = 301 | 302;
declare type EmptyResponseStatuses = 201 | 204;
declare type KnownJsonResponseTypes = "application/json" | "application/scim+json" | "text/html";
declare type SuccessResponseDataType<Responses> = {
    [K in SuccessStatuses & keyof Responses]: GetContentKeyIfPresent<Responses[K]> extends never ? never : OctokitResponse<GetContentKeyIfPresent<Responses[K]>, K>;
}[SuccessStatuses & keyof Responses];
declare type RedirectResponseDataType<Responses> = {
    [K in RedirectStatuses & keyof Responses]: OctokitResponse<unknown, K>;
}[RedirectStatuses & keyof Responses];
declare type EmptyResponseDataType<Responses> = {
    [K in EmptyResponseStatuses & keyof Responses]: OctokitResponse<never, K>;
}[EmptyResponseStatuses & keyof Responses];
declare type GetContentKeyIfPresent<T> = "content" extends keyof T ? DataType<T["content"]> : DataType<T>;
declare type DataType<T> = {
    [K in KnownJsonResponseTypes & keyof T]: T[K];
}[KnownJsonResponseTypes & keyof T];
declare type ExtractOctokitResponse<R> = "responses" extends keyof R ? SuccessResponseDataType<R["responses"]> extends never ? RedirectResponseDataType<R["responses"]> extends never ? EmptyResponseDataType<R["responses"]> : RedirectResponseDataType<R["responses"]> : SuccessResponseDataType<R["responses"]> : unknown;
export interface Endpoints {
    /**
     * @see https://docs.github.com/rest/reference/apps#delete-an-installation-for-the-authenticated-app
     */
    "DELETE /app/installations/{installation_id}": Operation<"/app/installations/{installation_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/apps#unsuspend-an-app-installation
     */
    "DELETE /app/installations/{installation_id}/suspended": Operation<"/app/installations/{installation_id}/suspended", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#delete-a-grant
     */
    "DELETE /applications/grants/{grant_id}": Operation<"/applications/grants/{grant_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/apps#delete-an-app-authorization
     */
    "DELETE /applications/{client_id}/grant": Operation<"/applications/{client_id}/grant", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/apps#delete-an-app-token
     */
    "DELETE /applications/{client_id}/token": Operation<"/applications/{client_id}/token", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#delete-an-authorization
     */
    "DELETE /authorizations/{authorization_id}": Operation<"/authorizations/{authorization_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#disable-a-selected-organization-for-github-actions-in-an-enterprise
     */
    "DELETE /enterprises/{enterprise}/actions/permissions/organizations/{org_id}": Operation<"/enterprises/{enterprise}/actions/permissions/organizations/{org_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#delete-a-self-hosted-runner-group-from-an-enterprise
     */
    "DELETE /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#remove-organization-access-to-a-self-hosted-runner-group-in-an-enterprise
     */
    "DELETE /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations/{org_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations/{org_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#remove-a-self-hosted-runner-from-a-group-for-an-enterprise
     */
    "DELETE /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners/{runner_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners/{runner_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#delete-self-hosted-runner-from-an-enterprise
     */
    "DELETE /enterprises/{enterprise}/actions/runners/{runner_id}": Operation<"/enterprises/{enterprise}/actions/runners/{runner_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/gists#delete-a-gist
     */
    "DELETE /gists/{gist_id}": Operation<"/gists/{gist_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/gists#delete-a-gist-comment
     */
    "DELETE /gists/{gist_id}/comments/{comment_id}": Operation<"/gists/{gist_id}/comments/{comment_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/gists#unstar-a-gist
     */
    "DELETE /gists/{gist_id}/star": Operation<"/gists/{gist_id}/star", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/apps#revoke-an-installation-access-token
     */
    "DELETE /installation/token": Operation<"/installation/token", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/activity#delete-a-thread-subscription
     */
    "DELETE /notifications/threads/{thread_id}/subscription": Operation<"/notifications/threads/{thread_id}/subscription", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#disable-a-selected-repository-for-github-actions-in-an-organization
     */
    "DELETE /orgs/{org}/actions/permissions/repositories/{repository_id}": Operation<"/orgs/{org}/actions/permissions/repositories/{repository_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-a-self-hosted-runner-group-from-an-organization
     */
    "DELETE /orgs/{org}/actions/runner-groups/{runner_group_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#remove-repository-access-to-a-self-hosted-runner-group-in-an-organization
     */
    "DELETE /orgs/{org}/actions/runner-groups/{runner_group_id}/repositories/{repository_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/repositories/{repository_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#remove-a-self-hosted-runner-from-a-group-for-an-organization
     */
    "DELETE /orgs/{org}/actions/runner-groups/{runner_group_id}/runners/{runner_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/runners/{runner_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-a-self-hosted-runner-from-an-organization
     */
    "DELETE /orgs/{org}/actions/runners/{runner_id}": Operation<"/orgs/{org}/actions/runners/{runner_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-an-organization-secret
     */
    "DELETE /orgs/{org}/actions/secrets/{secret_name}": Operation<"/orgs/{org}/actions/secrets/{secret_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#remove-selected-repository-from-an-organization-secret
     */
    "DELETE /orgs/{org}/actions/secrets/{secret_name}/repositories/{repository_id}": Operation<"/orgs/{org}/actions/secrets/{secret_name}/repositories/{repository_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#unblock-a-user-from-an-organization
     */
    "DELETE /orgs/{org}/blocks/{username}": Operation<"/orgs/{org}/blocks/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#remove-a-saml-sso-authorization-for-an-organization
     */
    "DELETE /orgs/{org}/credential-authorizations/{credential_id}": Operation<"/orgs/{org}/credential-authorizations/{credential_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#delete-an-organization-webhook
     */
    "DELETE /orgs/{org}/hooks/{hook_id}": Operation<"/orgs/{org}/hooks/{hook_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#remove-interaction-restrictions-for-an-organization
     */
    "DELETE /orgs/{org}/interaction-limits": Operation<"/orgs/{org}/interaction-limits", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#cancel-an-organization-invitation
     */
    "DELETE /orgs/{org}/invitations/{invitation_id}": Operation<"/orgs/{org}/invitations/{invitation_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#remove-an-organization-member
     */
    "DELETE /orgs/{org}/members/{username}": Operation<"/orgs/{org}/members/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#remove-organization-membership-for-a-user
     */
    "DELETE /orgs/{org}/memberships/{username}": Operation<"/orgs/{org}/memberships/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#delete-an-organization-migration-archive
     */
    "DELETE /orgs/{org}/migrations/{migration_id}/archive": Operation<"/orgs/{org}/migrations/{migration_id}/archive", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#unlock-an-organization-repository
     */
    "DELETE /orgs/{org}/migrations/{migration_id}/repos/{repo_name}/lock": Operation<"/orgs/{org}/migrations/{migration_id}/repos/{repo_name}/lock", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#remove-outside-collaborator-from-an-organization
     */
    "DELETE /orgs/{org}/outside_collaborators/{username}": Operation<"/orgs/{org}/outside_collaborators/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-for-an-organization
     */
    "DELETE /orgs/{org}/packages/{package_type}/{package_name}": Operation<"/orgs/{org}/packages/{package_type}/{package_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-version-for-an-organization
     */
    "DELETE /orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#remove-public-organization-membership-for-the-authenticated-user
     */
    "DELETE /orgs/{org}/public_members/{username}": Operation<"/orgs/{org}/public_members/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#delete-a-team
     */
    "DELETE /orgs/{org}/teams/{team_slug}": Operation<"/orgs/{org}/teams/{team_slug}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#delete-a-discussion
     */
    "DELETE /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#delete-a-discussion-comment
     */
    "DELETE /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-team-discussion-comment-reaction
     */
    "DELETE /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions/{reaction_id}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-team-discussion-reaction
     */
    "DELETE /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions/{reaction_id}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#remove-team-membership-for-a-user
     */
    "DELETE /orgs/{org}/teams/{team_slug}/memberships/{username}": Operation<"/orgs/{org}/teams/{team_slug}/memberships/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#remove-a-project-from-a-team
     */
    "DELETE /orgs/{org}/teams/{team_slug}/projects/{project_id}": Operation<"/orgs/{org}/teams/{team_slug}/projects/{project_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#remove-a-repository-from-a-team
     */
    "DELETE /orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}": Operation<"/orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/projects#delete-a-project-card
     */
    "DELETE /projects/columns/cards/{card_id}": Operation<"/projects/columns/cards/{card_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/projects#delete-a-project-column
     */
    "DELETE /projects/columns/{column_id}": Operation<"/projects/columns/{column_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/projects#delete-a-project
     */
    "DELETE /projects/{project_id}": Operation<"/projects/{project_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/projects#remove-project-collaborator
     */
    "DELETE /projects/{project_id}/collaborators/{username}": Operation<"/projects/{project_id}/collaborators/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#delete-a-reaction-legacy
     */
    "DELETE /reactions/{reaction_id}": Operation<"/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-repository
     */
    "DELETE /repos/{owner}/{repo}": Operation<"/repos/{owner}/{repo}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-an-artifact
     */
    "DELETE /repos/{owner}/{repo}/actions/artifacts/{artifact_id}": Operation<"/repos/{owner}/{repo}/actions/artifacts/{artifact_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-a-self-hosted-runner-from-a-repository
     */
    "DELETE /repos/{owner}/{repo}/actions/runners/{runner_id}": Operation<"/repos/{owner}/{repo}/actions/runners/{runner_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-a-workflow-run
     */
    "DELETE /repos/{owner}/{repo}/actions/runs/{run_id}": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-workflow-run-logs
     */
    "DELETE /repos/{owner}/{repo}/actions/runs/{run_id}/logs": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/logs", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-a-repository-secret
     */
    "DELETE /repos/{owner}/{repo}/actions/secrets/{secret_name}": Operation<"/repos/{owner}/{repo}/actions/secrets/{secret_name}", "delete">;
    /**
     * @see https://docs.github.com/v3/repos#delete-autolink
     */
    "DELETE /repos/{owner}/{repo}/autolinks/{autolink_id}": Operation<"/repos/{owner}/{repo}/autolinks/{autolink_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#disable-automated-security-fixes
     */
    "DELETE /repos/{owner}/{repo}/automated-security-fixes": Operation<"/repos/{owner}/{repo}/automated-security-fixes", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-branch-protection
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-admin-branch-protection
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-pull-request-review-protection
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-commit-signature-protection
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/required_signatures": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_signatures", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-status-check-protection
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-status-check-contexts
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-access-restrictions
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/restrictions": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-app-access-restrictions
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-team-access-restrictions
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-user-access-restrictions
     */
    "DELETE /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#delete-a-code-scanning-analysis-from-a-repository
     */
    "DELETE /repos/{owner}/{repo}/code-scanning/analyses/{analysis_id}{?confirm_delete}": Operation<"/repos/{owner}/{repo}/code-scanning/analyses/{analysis_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#remove-a-repository-collaborator
     */
    "DELETE /repos/{owner}/{repo}/collaborators/{username}": Operation<"/repos/{owner}/{repo}/collaborators/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-commit-comment
     */
    "DELETE /repos/{owner}/{repo}/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/comments/{comment_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-a-commit-comment-reaction
     */
    "DELETE /repos/{owner}/{repo}/comments/{comment_id}/reactions/{reaction_id}": Operation<"/repos/{owner}/{repo}/comments/{comment_id}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-file
     */
    "DELETE /repos/{owner}/{repo}/contents/{path}": Operation<"/repos/{owner}/{repo}/contents/{path}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-deployment
     */
    "DELETE /repos/{owner}/{repo}/deployments/{deployment_id}": Operation<"/repos/{owner}/{repo}/deployments/{deployment_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-an-environment
     */
    "DELETE /repos/{owner}/{repo}/environments/{environment_name}": Operation<"/repos/{owner}/{repo}/environments/{environment_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/git#delete-a-reference
     */
    "DELETE /repos/{owner}/{repo}/git/refs/{ref}": Operation<"/repos/{owner}/{repo}/git/refs/{ref}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-repository-webhook
     */
    "DELETE /repos/{owner}/{repo}/hooks/{hook_id}": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#cancel-an-import
     */
    "DELETE /repos/{owner}/{repo}/import": Operation<"/repos/{owner}/{repo}/import", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#remove-interaction-restrictions-for-a-repository
     */
    "DELETE /repos/{owner}/{repo}/interaction-limits": Operation<"/repos/{owner}/{repo}/interaction-limits", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-repository-invitation
     */
    "DELETE /repos/{owner}/{repo}/invitations/{invitation_id}": Operation<"/repos/{owner}/{repo}/invitations/{invitation_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#delete-an-issue-comment
     */
    "DELETE /repos/{owner}/{repo}/issues/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-an-issue-comment-reaction
     */
    "DELETE /repos/{owner}/{repo}/issues/comments/{comment_id}/reactions/{reaction_id}": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#remove-assignees-from-an-issue
     */
    "DELETE /repos/{owner}/{repo}/issues/{issue_number}/assignees": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/assignees", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#remove-all-labels-from-an-issue
     */
    "DELETE /repos/{owner}/{repo}/issues/{issue_number}/labels": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/labels", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#remove-a-label-from-an-issue
     */
    "DELETE /repos/{owner}/{repo}/issues/{issue_number}/labels/{name}": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/labels/{name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#unlock-an-issue
     */
    "DELETE /repos/{owner}/{repo}/issues/{issue_number}/lock": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/lock", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-an-issue-reaction
     */
    "DELETE /repos/{owner}/{repo}/issues/{issue_number}/reactions/{reaction_id}": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-deploy-key
     */
    "DELETE /repos/{owner}/{repo}/keys/{key_id}": Operation<"/repos/{owner}/{repo}/keys/{key_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#delete-a-label
     */
    "DELETE /repos/{owner}/{repo}/labels/{name}": Operation<"/repos/{owner}/{repo}/labels/{name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#disable-git-lfs-for-a-repository
     */
    "DELETE /repos/{owner}/{repo}/lfs": Operation<"/repos/{owner}/{repo}/lfs", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/issues#delete-a-milestone
     */
    "DELETE /repos/{owner}/{repo}/milestones/{milestone_number}": Operation<"/repos/{owner}/{repo}/milestones/{milestone_number}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-github-pages-site
     */
    "DELETE /repos/{owner}/{repo}/pages": Operation<"/repos/{owner}/{repo}/pages", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#delete-a-review-comment-for-a-pull-request
     */
    "DELETE /repos/{owner}/{repo}/pulls/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#delete-a-pull-request-comment-reaction
     */
    "DELETE /repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions/{reaction_id}": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions/{reaction_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#remove-requested-reviewers-from-a-pull-request
     */
    "DELETE /repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#delete-a-pending-review-for-a-pull-request
     */
    "DELETE /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-release-asset
     */
    "DELETE /repos/{owner}/{repo}/releases/assets/{asset_id}": Operation<"/repos/{owner}/{repo}/releases/assets/{asset_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#delete-a-release
     */
    "DELETE /repos/{owner}/{repo}/releases/{release_id}": Operation<"/repos/{owner}/{repo}/releases/{release_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/activity#delete-a-repository-subscription
     */
    "DELETE /repos/{owner}/{repo}/subscription": Operation<"/repos/{owner}/{repo}/subscription", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#disable-vulnerability-alerts
     */
    "DELETE /repos/{owner}/{repo}/vulnerability-alerts": Operation<"/repos/{owner}/{repo}/vulnerability-alerts", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/actions#delete-an-environment-secret
     */
    "DELETE /repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}": Operation<"/repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#delete-a-scim-group-from-an-enterprise
     */
    "DELETE /scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}": Operation<"/scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#delete-a-scim-user-from-an-enterprise
     */
    "DELETE /scim/v2/enterprises/{enterprise}/Users/{scim_user_id}": Operation<"/scim/v2/enterprises/{enterprise}/Users/{scim_user_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/scim#delete-a-scim-user-from-an-organization
     */
    "DELETE /scim/v2/organizations/{org}/Users/{scim_user_id}": Operation<"/scim/v2/organizations/{org}/Users/{scim_user_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#delete-a-team-legacy
     */
    "DELETE /teams/{team_id}": Operation<"/teams/{team_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#delete-a-discussion-legacy
     */
    "DELETE /teams/{team_id}/discussions/{discussion_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#delete-a-discussion-comment-legacy
     */
    "DELETE /teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#remove-team-member-legacy
     */
    "DELETE /teams/{team_id}/members/{username}": Operation<"/teams/{team_id}/members/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams#remove-team-membership-for-a-user-legacy
     */
    "DELETE /teams/{team_id}/memberships/{username}": Operation<"/teams/{team_id}/memberships/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#remove-a-project-from-a-team-legacy
     */
    "DELETE /teams/{team_id}/projects/{project_id}": Operation<"/teams/{team_id}/projects/{project_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#remove-a-repository-from-a-team-legacy
     */
    "DELETE /teams/{team_id}/repos/{owner}/{repo}": Operation<"/teams/{team_id}/repos/{owner}/{repo}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/users#unblock-a-user
     */
    "DELETE /user/blocks/{username}": Operation<"/user/blocks/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/users#delete-an-email-address-for-the-authenticated-user
     */
    "DELETE /user/emails": Operation<"/user/emails", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/users#unfollow-a-user
     */
    "DELETE /user/following/{username}": Operation<"/user/following/{username}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/users#delete-a-gpg-key-for-the-authenticated-user
     */
    "DELETE /user/gpg_keys/{gpg_key_id}": Operation<"/user/gpg_keys/{gpg_key_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/apps#remove-a-repository-from-an-app-installation
     */
    "DELETE /user/installations/{installation_id}/repositories/{repository_id}": Operation<"/user/installations/{installation_id}/repositories/{repository_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#remove-interaction-restrictions-from-your-public-repositories
     */
    "DELETE /user/interaction-limits": Operation<"/user/interaction-limits", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/users#delete-a-public-ssh-key-for-the-authenticated-user
     */
    "DELETE /user/keys/{key_id}": Operation<"/user/keys/{key_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#delete-a-user-migration-archive
     */
    "DELETE /user/migrations/{migration_id}/archive": Operation<"/user/migrations/{migration_id}/archive", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#unlock-a-user-repository
     */
    "DELETE /user/migrations/{migration_id}/repos/{repo_name}/lock": Operation<"/user/migrations/{migration_id}/repos/{repo_name}/lock", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-for-the-authenticated-user
     */
    "DELETE /user/packages/{package_type}/{package_name}": Operation<"/user/packages/{package_type}/{package_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-version-for-the-authenticated-user
     */
    "DELETE /user/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/user/packages/{package_type}/{package_name}/versions/{package_version_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/repos#decline-a-repository-invitation
     */
    "DELETE /user/repository_invitations/{invitation_id}": Operation<"/user/repository_invitations/{invitation_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/activity#unstar-a-repository-for-the-authenticated-user
     */
    "DELETE /user/starred/{owner}/{repo}": Operation<"/user/starred/{owner}/{repo}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-for-a-user
     */
    "DELETE /users/{username}/packages/{package_type}/{package_name}": Operation<"/users/{username}/packages/{package_type}/{package_name}", "delete">;
    /**
     * @see https://docs.github.com/rest/reference/packages#delete-a-package-version-for-a-user
     */
    "DELETE /users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}", "delete">;
    /**
     * @see https://docs.github.com/rest/overview/resources-in-the-rest-api#root-endpoint
     */
    "GET /": Operation<"/", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-the-authenticated-app
     */
    "GET /app": Operation<"/app", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-webhook-configuration-for-an-app
     */
    "GET /app/hook/config": Operation<"/app/hook/config", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-deliveries-for-an-app-webhook
     */
    "GET /app/hook/deliveries": Operation<"/app/hook/deliveries", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-delivery-for-an-app-webhook
     */
    "GET /app/hook/deliveries/{delivery_id}": Operation<"/app/hook/deliveries/{delivery_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-installations-for-the-authenticated-app
     */
    "GET /app/installations": Operation<"/app/installations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-an-installation-for-the-authenticated-app
     */
    "GET /app/installations/{installation_id}": Operation<"/app/installations/{installation_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#list-your-grants
     */
    "GET /applications/grants": Operation<"/applications/grants", "get">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#get-a-single-grant
     */
    "GET /applications/grants/{grant_id}": Operation<"/applications/grants/{grant_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps/#get-an-app
     */
    "GET /apps/{app_slug}": Operation<"/apps/{app_slug}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#list-your-authorizations
     */
    "GET /authorizations": Operation<"/authorizations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#get-a-single-authorization
     */
    "GET /authorizations/{authorization_id}": Operation<"/authorizations/{authorization_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/codes-of-conduct#get-all-codes-of-conduct
     */
    "GET /codes_of_conduct": Operation<"/codes_of_conduct", "get">;
    /**
     * @see https://docs.github.com/rest/reference/codes-of-conduct#get-a-code-of-conduct
     */
    "GET /codes_of_conduct/{key}": Operation<"/codes_of_conduct/{key}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/emojis#get-emojis
     */
    "GET /emojis": Operation<"/emojis", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-github-actions-permissions-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/permissions": Operation<"/enterprises/{enterprise}/actions/permissions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-selected-organizations-enabled-for-github-actions-in-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/permissions/organizations": Operation<"/enterprises/{enterprise}/actions/permissions/organizations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-allowed-actions-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/permissions/selected-actions": Operation<"/enterprises/{enterprise}/actions/permissions/selected-actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-self-hosted-runner-groups-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runner-groups": Operation<"/enterprises/{enterprise}/actions/runner-groups", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-a-self-hosted-runner-group-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-organization-access-to-a-self-hosted-runner-group-in-a-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-self-hosted-runners-in-a-group-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-self-hosted-runners-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runners": Operation<"/enterprises/{enterprise}/actions/runners", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-runner-applications-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runners/downloads": Operation<"/enterprises/{enterprise}/actions/runners/downloads", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-a-self-hosted-runner-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/actions/runners/{runner_id}": Operation<"/enterprises/{enterprise}/actions/runners/{runner_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-the-audit-log-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/audit-log": Operation<"/enterprises/{enterprise}/audit-log", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-actions-billing-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/settings/billing/actions": Operation<"/enterprises/{enterprise}/settings/billing/actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-packages-billing-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/settings/billing/packages": Operation<"/enterprises/{enterprise}/settings/billing/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-shared-storage-billing-for-an-enterprise
     */
    "GET /enterprises/{enterprise}/settings/billing/shared-storage": Operation<"/enterprises/{enterprise}/settings/billing/shared-storage", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-public-events
     */
    "GET /events": Operation<"/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#get-feeds
     */
    "GET /feeds": Operation<"/feeds", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-gists-for-the-authenticated-user
     */
    "GET /gists": Operation<"/gists", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-public-gists
     */
    "GET /gists/public": Operation<"/gists/public", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-starred-gists
     */
    "GET /gists/starred": Operation<"/gists/starred", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#get-a-gist
     */
    "GET /gists/{gist_id}": Operation<"/gists/{gist_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-gist-comments
     */
    "GET /gists/{gist_id}/comments": Operation<"/gists/{gist_id}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#get-a-gist-comment
     */
    "GET /gists/{gist_id}/comments/{comment_id}": Operation<"/gists/{gist_id}/comments/{comment_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-gist-commits
     */
    "GET /gists/{gist_id}/commits": Operation<"/gists/{gist_id}/commits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-gist-forks
     */
    "GET /gists/{gist_id}/forks": Operation<"/gists/{gist_id}/forks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#check-if-a-gist-is-starred
     */
    "GET /gists/{gist_id}/star": Operation<"/gists/{gist_id}/star", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#get-a-gist-revision
     */
    "GET /gists/{gist_id}/{sha}": Operation<"/gists/{gist_id}/{sha}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gitignore#get-all-gitignore-templates
     */
    "GET /gitignore/templates": Operation<"/gitignore/templates", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gitignore#get-a-gitignore-template
     */
    "GET /gitignore/templates/{name}": Operation<"/gitignore/templates/{name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-repositories-accessible-to-the-app-installation
     */
    "GET /installation/repositories": Operation<"/installation/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-issues-assigned-to-the-authenticated-user
     */
    "GET /issues": Operation<"/issues", "get">;
    /**
     * @see https://docs.github.com/rest/reference/licenses#get-all-commonly-used-licenses
     */
    "GET /licenses": Operation<"/licenses", "get">;
    /**
     * @see https://docs.github.com/rest/reference/licenses#get-a-license
     */
    "GET /licenses/{license}": Operation<"/licenses/{license}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-subscription-plan-for-an-account
     */
    "GET /marketplace_listing/accounts/{account_id}": Operation<"/marketplace_listing/accounts/{account_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-plans
     */
    "GET /marketplace_listing/plans": Operation<"/marketplace_listing/plans", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-accounts-for-a-plan
     */
    "GET /marketplace_listing/plans/{plan_id}/accounts": Operation<"/marketplace_listing/plans/{plan_id}/accounts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-subscription-plan-for-an-account-stubbed
     */
    "GET /marketplace_listing/stubbed/accounts/{account_id}": Operation<"/marketplace_listing/stubbed/accounts/{account_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-plans-stubbed
     */
    "GET /marketplace_listing/stubbed/plans": Operation<"/marketplace_listing/stubbed/plans", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-accounts-for-a-plan-stubbed
     */
    "GET /marketplace_listing/stubbed/plans/{plan_id}/accounts": Operation<"/marketplace_listing/stubbed/plans/{plan_id}/accounts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/meta#get-github-meta-information
     */
    "GET /meta": Operation<"/meta", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-public-events-for-a-network-of-repositories
     */
    "GET /networks/{owner}/{repo}/events": Operation<"/networks/{owner}/{repo}/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-notifications-for-the-authenticated-user
     */
    "GET /notifications": Operation<"/notifications", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#get-a-thread
     */
    "GET /notifications/threads/{thread_id}": Operation<"/notifications/threads/{thread_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#get-a-thread-subscription-for-the-authenticated-user
     */
    "GET /notifications/threads/{thread_id}/subscription": Operation<"/notifications/threads/{thread_id}/subscription", "get">;
    /**
     * @see https://docs.github.com/rest/reference/meta#get-octocat
     */
    "GET /octocat": Operation<"/octocat", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organizations
     */
    "GET /organizations": Operation<"/organizations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-an-organization
     */
    "GET /orgs/{org}": Operation<"/orgs/{org}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-github-actions-permissions-for-an-organization
     */
    "GET /orgs/{org}/actions/permissions": Operation<"/orgs/{org}/actions/permissions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-selected-repositories-enabled-for-github-actions-in-an-organization
     */
    "GET /orgs/{org}/actions/permissions/repositories": Operation<"/orgs/{org}/actions/permissions/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-allowed-actions-for-an-organization
     */
    "GET /orgs/{org}/actions/permissions/selected-actions": Operation<"/orgs/{org}/actions/permissions/selected-actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-self-hosted-runner-groups-for-an-organization
     */
    "GET /orgs/{org}/actions/runner-groups": Operation<"/orgs/{org}/actions/runner-groups", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-self-hosted-runner-group-for-an-organization
     */
    "GET /orgs/{org}/actions/runner-groups/{runner_group_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-repository-access-to-a-self-hosted-runner-group-in-an-organization
     */
    "GET /orgs/{org}/actions/runner-groups/{runner_group_id}/repositories": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-self-hosted-runners-in-a-group-for-an-organization
     */
    "GET /orgs/{org}/actions/runner-groups/{runner_group_id}/runners": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/runners", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-self-hosted-runners-for-an-organization
     */
    "GET /orgs/{org}/actions/runners": Operation<"/orgs/{org}/actions/runners", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-runner-applications-for-an-organization
     */
    "GET /orgs/{org}/actions/runners/downloads": Operation<"/orgs/{org}/actions/runners/downloads", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-self-hosted-runner-for-an-organization
     */
    "GET /orgs/{org}/actions/runners/{runner_id}": Operation<"/orgs/{org}/actions/runners/{runner_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-organization-secrets
     */
    "GET /orgs/{org}/actions/secrets": Operation<"/orgs/{org}/actions/secrets", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-an-organization-public-key
     */
    "GET /orgs/{org}/actions/secrets/public-key": Operation<"/orgs/{org}/actions/secrets/public-key", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-an-organization-secret
     */
    "GET /orgs/{org}/actions/secrets/{secret_name}": Operation<"/orgs/{org}/actions/secrets/{secret_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-selected-repositories-for-an-organization-secret
     */
    "GET /orgs/{org}/actions/secrets/{secret_name}/repositories": Operation<"/orgs/{org}/actions/secrets/{secret_name}/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-audit-log
     */
    "GET /orgs/{org}/audit-log": Operation<"/orgs/{org}/audit-log", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-users-blocked-by-an-organization
     */
    "GET /orgs/{org}/blocks": Operation<"/orgs/{org}/blocks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#check-if-a-user-is-blocked-by-an-organization
     */
    "GET /orgs/{org}/blocks/{username}": Operation<"/orgs/{org}/blocks/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-saml-sso-authorizations-for-an-organization
     */
    "GET /orgs/{org}/credential-authorizations": Operation<"/orgs/{org}/credential-authorizations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-public-organization-events
     */
    "GET /orgs/{org}/events": Operation<"/orgs/{org}/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-failed-organization-invitations
     */
    "GET /orgs/{org}/failed_invitations": Operation<"/orgs/{org}/failed_invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organization-webhooks
     */
    "GET /orgs/{org}/hooks": Operation<"/orgs/{org}/hooks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-an-organization-webhook
     */
    "GET /orgs/{org}/hooks/{hook_id}": Operation<"/orgs/{org}/hooks/{hook_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-a-webhook-configuration-for-an-organization
     */
    "GET /orgs/{org}/hooks/{hook_id}/config": Operation<"/orgs/{org}/hooks/{hook_id}/config", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-deliveries-for-an-organization-webhook
     */
    "GET /orgs/{org}/hooks/{hook_id}/deliveries": Operation<"/orgs/{org}/hooks/{hook_id}/deliveries", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-a-webhook-delivery-for-an-organization-webhook
     */
    "GET /orgs/{org}/hooks/{hook_id}/deliveries/{delivery_id}": Operation<"/orgs/{org}/hooks/{hook_id}/deliveries/{delivery_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-an-organization-installation-for-the-authenticated-app
     */
    "GET /orgs/{org}/installation": Operation<"/orgs/{org}/installation", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-app-installations-for-an-organization
     */
    "GET /orgs/{org}/installations": Operation<"/orgs/{org}/installations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#get-interaction-restrictions-for-an-organization
     */
    "GET /orgs/{org}/interaction-limits": Operation<"/orgs/{org}/interaction-limits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-pending-organization-invitations
     */
    "GET /orgs/{org}/invitations": Operation<"/orgs/{org}/invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organization-invitation-teams
     */
    "GET /orgs/{org}/invitations/{invitation_id}/teams": Operation<"/orgs/{org}/invitations/{invitation_id}/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-organization-issues-assigned-to-the-authenticated-user
     */
    "GET /orgs/{org}/issues": Operation<"/orgs/{org}/issues", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organization-members
     */
    "GET /orgs/{org}/members": Operation<"/orgs/{org}/members", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#check-organization-membership-for-a-user
     */
    "GET /orgs/{org}/members/{username}": Operation<"/orgs/{org}/members/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-organization-membership-for-a-user
     */
    "GET /orgs/{org}/memberships/{username}": Operation<"/orgs/{org}/memberships/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#list-organization-migrations
     */
    "GET /orgs/{org}/migrations": Operation<"/orgs/{org}/migrations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#get-an-organization-migration-status
     */
    "GET /orgs/{org}/migrations/{migration_id}": Operation<"/orgs/{org}/migrations/{migration_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#download-an-organization-migration-archive
     */
    "GET /orgs/{org}/migrations/{migration_id}/archive": Operation<"/orgs/{org}/migrations/{migration_id}/archive", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#list-repositories-in-an-organization-migration
     */
    "GET /orgs/{org}/migrations/{migration_id}/repositories": Operation<"/orgs/{org}/migrations/{migration_id}/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-outside-collaborators-for-an-organization
     */
    "GET /orgs/{org}/outside_collaborators": Operation<"/orgs/{org}/outside_collaborators", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#list-packages-for-an-organization
     */
    "GET /orgs/{org}/packages": Operation<"/orgs/{org}/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-for-an-organization
     */
    "GET /orgs/{org}/packages/{package_type}/{package_name}": Operation<"/orgs/{org}/packages/{package_type}/{package_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-all-package-versions-for-a-package-owned-by-an-organization
     */
    "GET /orgs/{org}/packages/{package_type}/{package_name}/versions": Operation<"/orgs/{org}/packages/{package_type}/{package_name}/versions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-version-for-an-organization
     */
    "GET /orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-organization-projects
     */
    "GET /orgs/{org}/projects": Operation<"/orgs/{org}/projects", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-public-organization-members
     */
    "GET /orgs/{org}/public_members": Operation<"/orgs/{org}/public_members", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#check-public-organization-membership-for-a-user
     */
    "GET /orgs/{org}/public_members/{username}": Operation<"/orgs/{org}/public_members/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-organization-repositories
     */
    "GET /orgs/{org}/repos": Operation<"/orgs/{org}/repos", "get">;
    /**
     * @see https://docs.github.com/rest/reference/secret-scanning#list-secret-scanning-alerts-by-organization
     */
    "GET /orgs/{org}/secret-scanning/alerts": Operation<"/orgs/{org}/secret-scanning/alerts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-actions-billing-for-an-organization
     */
    "GET /orgs/{org}/settings/billing/actions": Operation<"/orgs/{org}/settings/billing/actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-packages-billing-for-an-organization
     */
    "GET /orgs/{org}/settings/billing/packages": Operation<"/orgs/{org}/settings/billing/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-shared-storage-billing-for-an-organization
     */
    "GET /orgs/{org}/settings/billing/shared-storage": Operation<"/orgs/{org}/settings/billing/shared-storage", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-idp-groups-for-an-organization
     */
    "GET /orgs/{org}/team-sync/groups": Operation<"/orgs/{org}/team-sync/groups", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-teams
     */
    "GET /orgs/{org}/teams": Operation<"/orgs/{org}/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-a-team-by-name
     */
    "GET /orgs/{org}/teams/{team_slug}": Operation<"/orgs/{org}/teams/{team_slug}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-discussions
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions": Operation<"/orgs/{org}/teams/{team_slug}/discussions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-a-discussion
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-discussion-comments
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-a-discussion-comment
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-a-team-discussion-comment
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-a-team-discussion
     */
    "GET /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-pending-team-invitations
     */
    "GET /orgs/{org}/teams/{team_slug}/invitations": Operation<"/orgs/{org}/teams/{team_slug}/invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-team-members
     */
    "GET /orgs/{org}/teams/{team_slug}/members": Operation<"/orgs/{org}/teams/{team_slug}/members", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-team-membership-for-a-user
     */
    "GET /orgs/{org}/teams/{team_slug}/memberships/{username}": Operation<"/orgs/{org}/teams/{team_slug}/memberships/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-team-projects
     */
    "GET /orgs/{org}/teams/{team_slug}/projects": Operation<"/orgs/{org}/teams/{team_slug}/projects", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#check-team-permissions-for-a-project
     */
    "GET /orgs/{org}/teams/{team_slug}/projects/{project_id}": Operation<"/orgs/{org}/teams/{team_slug}/projects/{project_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-team-repositories
     */
    "GET /orgs/{org}/teams/{team_slug}/repos": Operation<"/orgs/{org}/teams/{team_slug}/repos", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#check-team-permissions-for-a-repository
     */
    "GET /orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}": Operation<"/orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-idp-groups-for-a-team
     */
    "GET /orgs/{org}/teams/{team_slug}/team-sync/group-mappings": Operation<"/orgs/{org}/teams/{team_slug}/team-sync/group-mappings", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-child-teams
     */
    "GET /orgs/{org}/teams/{team_slug}/teams": Operation<"/orgs/{org}/teams/{team_slug}/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#get-a-project-card
     */
    "GET /projects/columns/cards/{card_id}": Operation<"/projects/columns/cards/{card_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#get-a-project-column
     */
    "GET /projects/columns/{column_id}": Operation<"/projects/columns/{column_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-project-cards
     */
    "GET /projects/columns/{column_id}/cards": Operation<"/projects/columns/{column_id}/cards", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#get-a-project
     */
    "GET /projects/{project_id}": Operation<"/projects/{project_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-project-collaborators
     */
    "GET /projects/{project_id}/collaborators": Operation<"/projects/{project_id}/collaborators", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#get-project-permission-for-a-user
     */
    "GET /projects/{project_id}/collaborators/{username}/permission": Operation<"/projects/{project_id}/collaborators/{username}/permission", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-project-columns
     */
    "GET /projects/{project_id}/columns": Operation<"/projects/{project_id}/columns", "get">;
    /**
     * @see https://docs.github.com/rest/reference/rate-limit#get-rate-limit-status-for-the-authenticated-user
     */
    "GET /rate_limit": Operation<"/rate_limit", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-repository
     */
    "GET /repos/{owner}/{repo}": Operation<"/repos/{owner}/{repo}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-artifacts-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/artifacts": Operation<"/repos/{owner}/{repo}/actions/artifacts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-an-artifact
     */
    "GET /repos/{owner}/{repo}/actions/artifacts/{artifact_id}": Operation<"/repos/{owner}/{repo}/actions/artifacts/{artifact_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#download-an-artifact
     */
    "GET /repos/{owner}/{repo}/actions/artifacts/{artifact_id}/{archive_format}": Operation<"/repos/{owner}/{repo}/actions/artifacts/{artifact_id}/{archive_format}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-job-for-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/jobs/{job_id}": Operation<"/repos/{owner}/{repo}/actions/jobs/{job_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#download-job-logs-for-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs": Operation<"/repos/{owner}/{repo}/actions/jobs/{job_id}/logs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-github-actions-permissions-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/permissions": Operation<"/repos/{owner}/{repo}/actions/permissions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-allowed-actions-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/permissions/selected-actions": Operation<"/repos/{owner}/{repo}/actions/permissions/selected-actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-self-hosted-runners-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/runners": Operation<"/repos/{owner}/{repo}/actions/runners", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-runner-applications-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/runners/downloads": Operation<"/repos/{owner}/{repo}/actions/runners/downloads", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-self-hosted-runner-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/runners/{runner_id}": Operation<"/repos/{owner}/{repo}/actions/runners/{runner_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-workflow-runs-for-a-repository
     */
    "GET /repos/{owner}/{repo}/actions/runs": Operation<"/repos/{owner}/{repo}/actions/runs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-the-review-history-for-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/approvals": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/approvals", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-workflow-run-artifacts
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/artifacts": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/artifacts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-workflow-run-attempt
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-jobs-for-a-workflow-run-attempt
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}/jobs": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}/jobs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#download-workflow-run-attempt-logs
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}/logs": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}/logs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-jobs-for-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/jobs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#download-workflow-run-logs
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/logs": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/logs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-pending-deployments-for-a-workflow-run
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/pending_deployments": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/pending_deployments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-workflow-run-usage
     */
    "GET /repos/{owner}/{repo}/actions/runs/{run_id}/timing": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/timing", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-repository-secrets
     */
    "GET /repos/{owner}/{repo}/actions/secrets": Operation<"/repos/{owner}/{repo}/actions/secrets", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-repository-public-key
     */
    "GET /repos/{owner}/{repo}/actions/secrets/public-key": Operation<"/repos/{owner}/{repo}/actions/secrets/public-key", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-repository-secret
     */
    "GET /repos/{owner}/{repo}/actions/secrets/{secret_name}": Operation<"/repos/{owner}/{repo}/actions/secrets/{secret_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-repository-workflows
     */
    "GET /repos/{owner}/{repo}/actions/workflows": Operation<"/repos/{owner}/{repo}/actions/workflows", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-a-workflow
     */
    "GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-workflow-runs
     */
    "GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-workflow-usage
     */
    "GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/timing": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}/timing", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-assignees
     */
    "GET /repos/{owner}/{repo}/assignees": Operation<"/repos/{owner}/{repo}/assignees", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#check-if-a-user-can-be-assigned
     */
    "GET /repos/{owner}/{repo}/assignees/{assignee}": Operation<"/repos/{owner}/{repo}/assignees/{assignee}", "get">;
    /**
     * @see https://docs.github.com/v3/repos#list-autolinks
     */
    "GET /repos/{owner}/{repo}/autolinks": Operation<"/repos/{owner}/{repo}/autolinks", "get">;
    /**
     * @see https://docs.github.com/v3/repos#get-autolink
     */
    "GET /repos/{owner}/{repo}/autolinks/{autolink_id}": Operation<"/repos/{owner}/{repo}/autolinks/{autolink_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-branches
     */
    "GET /repos/{owner}/{repo}/branches": Operation<"/repos/{owner}/{repo}/branches", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-branch
     */
    "GET /repos/{owner}/{repo}/branches/{branch}": Operation<"/repos/{owner}/{repo}/branches/{branch}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-branch-protection
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-admin-branch-protection
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-pull-request-review-protection
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-commit-signature-protection
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/required_signatures": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_signatures", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-status-checks-protection
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-all-status-check-contexts
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-access-restrictions
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/restrictions": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-apps-with-access-to-the-protected-branch
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-teams-with-access-to-the-protected-branch
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-users-with-access-to-the-protected-branch
     */
    "GET /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#get-a-check-run
     */
    "GET /repos/{owner}/{repo}/check-runs/{check_run_id}": Operation<"/repos/{owner}/{repo}/check-runs/{check_run_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#list-check-run-annotations
     */
    "GET /repos/{owner}/{repo}/check-runs/{check_run_id}/annotations": Operation<"/repos/{owner}/{repo}/check-runs/{check_run_id}/annotations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#get-a-check-suite
     */
    "GET /repos/{owner}/{repo}/check-suites/{check_suite_id}": Operation<"/repos/{owner}/{repo}/check-suites/{check_suite_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#list-check-runs-in-a-check-suite
     */
    "GET /repos/{owner}/{repo}/check-suites/{check_suite_id}/check-runs": Operation<"/repos/{owner}/{repo}/check-suites/{check_suite_id}/check-runs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#list-code-scanning-alerts-for-a-repository
     */
    "GET /repos/{owner}/{repo}/code-scanning/alerts": Operation<"/repos/{owner}/{repo}/code-scanning/alerts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#get-a-code-scanning-alert
     * @deprecated "alert_id" is now "alert_number"
     */
    "GET /repos/{owner}/{repo}/code-scanning/alerts/{alert_id}": Operation<"/repos/{owner}/{repo}/code-scanning/alerts/{alert_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#get-a-code-scanning-alert
     */
    "GET /repos/{owner}/{repo}/code-scanning/alerts/{alert_number}": Operation<"/repos/{owner}/{repo}/code-scanning/alerts/{alert_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#list-instances-of-a-code-scanning-alert
     */
    "GET /repos/{owner}/{repo}/code-scanning/alerts/{alert_number}/instances": Operation<"/repos/{owner}/{repo}/code-scanning/alerts/{alert_number}/instances", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#list-code-scanning-analyses-for-a-repository
     */
    "GET /repos/{owner}/{repo}/code-scanning/analyses": Operation<"/repos/{owner}/{repo}/code-scanning/analyses", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#get-a-code-scanning-analysis-for-a-repository
     */
    "GET /repos/{owner}/{repo}/code-scanning/analyses/{analysis_id}": Operation<"/repos/{owner}/{repo}/code-scanning/analyses/{analysis_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#list-recent-code-scanning-analyses-for-a-repository
     */
    "GET /repos/{owner}/{repo}/code-scanning/sarifs/{sarif_id}": Operation<"/repos/{owner}/{repo}/code-scanning/sarifs/{sarif_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-collaborators
     */
    "GET /repos/{owner}/{repo}/collaborators": Operation<"/repos/{owner}/{repo}/collaborators", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#check-if-a-user-is-a-repository-collaborator
     */
    "GET /repos/{owner}/{repo}/collaborators/{username}": Operation<"/repos/{owner}/{repo}/collaborators/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-repository-permissions-for-a-user
     */
    "GET /repos/{owner}/{repo}/collaborators/{username}/permission": Operation<"/repos/{owner}/{repo}/collaborators/{username}/permission", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-commit-comments-for-a-repository
     */
    "GET /repos/{owner}/{repo}/comments": Operation<"/repos/{owner}/{repo}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-commit-comment
     */
    "GET /repos/{owner}/{repo}/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/comments/{comment_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-a-commit-comment
     */
    "GET /repos/{owner}/{repo}/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/comments/{comment_id}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-commits
     */
    "GET /repos/{owner}/{repo}/commits": Operation<"/repos/{owner}/{repo}/commits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-branches-for-head-commit
     */
    "GET /repos/{owner}/{repo}/commits/{commit_sha}/branches-where-head": Operation<"/repos/{owner}/{repo}/commits/{commit_sha}/branches-where-head", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-commit-comments
     */
    "GET /repos/{owner}/{repo}/commits/{commit_sha}/comments": Operation<"/repos/{owner}/{repo}/commits/{commit_sha}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-pull-requests-associated-with-a-commit
     */
    "GET /repos/{owner}/{repo}/commits/{commit_sha}/pulls": Operation<"/repos/{owner}/{repo}/commits/{commit_sha}/pulls", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-commit
     */
    "GET /repos/{owner}/{repo}/commits/{ref}": Operation<"/repos/{owner}/{repo}/commits/{ref}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#list-check-runs-for-a-git-reference
     */
    "GET /repos/{owner}/{repo}/commits/{ref}/check-runs": Operation<"/repos/{owner}/{repo}/commits/{ref}/check-runs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/checks#list-check-suites-for-a-git-reference
     */
    "GET /repos/{owner}/{repo}/commits/{ref}/check-suites": Operation<"/repos/{owner}/{repo}/commits/{ref}/check-suites", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-combined-status-for-a-specific-reference
     */
    "GET /repos/{owner}/{repo}/commits/{ref}/status": Operation<"/repos/{owner}/{repo}/commits/{ref}/status", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-commit-statuses-for-a-reference
     */
    "GET /repos/{owner}/{repo}/commits/{ref}/statuses": Operation<"/repos/{owner}/{repo}/commits/{ref}/statuses", "get">;
    /**
     * @see https://docs.github.com/rest/reference/codes-of-conduct#get-the-code-of-conduct-for-a-repository
     */
    "GET /repos/{owner}/{repo}/community/code_of_conduct": Operation<"/repos/{owner}/{repo}/community/code_of_conduct", "get", "scarlet-witch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-community-profile-metrics
     */
    "GET /repos/{owner}/{repo}/community/profile": Operation<"/repos/{owner}/{repo}/community/profile", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#compare-two-commits
     */
    "GET /repos/{owner}/{repo}/compare/{basehead}": Operation<"/repos/{owner}/{repo}/compare/{basehead}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#compare-two-commits
     */
    "GET /repos/{owner}/{repo}/compare/{base}...{head}": Operation<"/repos/{owner}/{repo}/compare/{base}...{head}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-repository-content
     */
    "GET /repos/{owner}/{repo}/contents/{path}": Operation<"/repos/{owner}/{repo}/contents/{path}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-contributors
     */
    "GET /repos/{owner}/{repo}/contributors": Operation<"/repos/{owner}/{repo}/contributors", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-deployments
     */
    "GET /repos/{owner}/{repo}/deployments": Operation<"/repos/{owner}/{repo}/deployments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-deployment
     */
    "GET /repos/{owner}/{repo}/deployments/{deployment_id}": Operation<"/repos/{owner}/{repo}/deployments/{deployment_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-deployment-statuses
     */
    "GET /repos/{owner}/{repo}/deployments/{deployment_id}/statuses": Operation<"/repos/{owner}/{repo}/deployments/{deployment_id}/statuses", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-deployment-status
     */
    "GET /repos/{owner}/{repo}/deployments/{deployment_id}/statuses/{status_id}": Operation<"/repos/{owner}/{repo}/deployments/{deployment_id}/statuses/{status_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-all-environments
     */
    "GET /repos/{owner}/{repo}/environments": Operation<"/repos/{owner}/{repo}/environments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-an-environment
     */
    "GET /repos/{owner}/{repo}/environments/{environment_name}": Operation<"/repos/{owner}/{repo}/environments/{environment_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repository-events
     */
    "GET /repos/{owner}/{repo}/events": Operation<"/repos/{owner}/{repo}/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-forks
     */
    "GET /repos/{owner}/{repo}/forks": Operation<"/repos/{owner}/{repo}/forks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#get-a-blob
     */
    "GET /repos/{owner}/{repo}/git/blobs/{file_sha}": Operation<"/repos/{owner}/{repo}/git/blobs/{file_sha}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#get-a-commit
     */
    "GET /repos/{owner}/{repo}/git/commits/{commit_sha}": Operation<"/repos/{owner}/{repo}/git/commits/{commit_sha}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#list-matching-references
     */
    "GET /repos/{owner}/{repo}/git/matching-refs/{ref}": Operation<"/repos/{owner}/{repo}/git/matching-refs/{ref}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#get-a-reference
     */
    "GET /repos/{owner}/{repo}/git/ref/{ref}": Operation<"/repos/{owner}/{repo}/git/ref/{ref}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#get-a-tag
     */
    "GET /repos/{owner}/{repo}/git/tags/{tag_sha}": Operation<"/repos/{owner}/{repo}/git/tags/{tag_sha}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/git#get-a-tree
     */
    "GET /repos/{owner}/{repo}/git/trees/{tree_sha}": Operation<"/repos/{owner}/{repo}/git/trees/{tree_sha}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-webhooks
     */
    "GET /repos/{owner}/{repo}/hooks": Operation<"/repos/{owner}/{repo}/hooks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-repository-webhook
     */
    "GET /repos/{owner}/{repo}/hooks/{hook_id}": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-webhook-configuration-for-a-repository
     */
    "GET /repos/{owner}/{repo}/hooks/{hook_id}/config": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/config", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-deliveries-for-a-repository-webhook
     */
    "GET /repos/{owner}/{repo}/hooks/{hook_id}/deliveries": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/deliveries", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-delivery-for-a-repository-webhook
     */
    "GET /repos/{owner}/{repo}/hooks/{hook_id}/deliveries/{delivery_id}": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/deliveries/{delivery_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#get-an-import-status
     */
    "GET /repos/{owner}/{repo}/import": Operation<"/repos/{owner}/{repo}/import", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#get-commit-authors
     */
    "GET /repos/{owner}/{repo}/import/authors": Operation<"/repos/{owner}/{repo}/import/authors", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#get-large-files
     */
    "GET /repos/{owner}/{repo}/import/large_files": Operation<"/repos/{owner}/{repo}/import/large_files", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-repository-installation-for-the-authenticated-app
     */
    "GET /repos/{owner}/{repo}/installation": Operation<"/repos/{owner}/{repo}/installation", "get">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#get-interaction-restrictions-for-a-repository
     */
    "GET /repos/{owner}/{repo}/interaction-limits": Operation<"/repos/{owner}/{repo}/interaction-limits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-invitations
     */
    "GET /repos/{owner}/{repo}/invitations": Operation<"/repos/{owner}/{repo}/invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-repository-issues
     */
    "GET /repos/{owner}/{repo}/issues": Operation<"/repos/{owner}/{repo}/issues", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-issue-comments-for-a-repository
     */
    "GET /repos/{owner}/{repo}/issues/comments": Operation<"/repos/{owner}/{repo}/issues/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#get-an-issue-comment
     */
    "GET /repos/{owner}/{repo}/issues/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-an-issue-comment
     */
    "GET /repos/{owner}/{repo}/issues/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-issue-events-for-a-repository
     */
    "GET /repos/{owner}/{repo}/issues/events": Operation<"/repos/{owner}/{repo}/issues/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#get-an-issue-event
     */
    "GET /repos/{owner}/{repo}/issues/events/{event_id}": Operation<"/repos/{owner}/{repo}/issues/events/{event_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#get-an-issue
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}": Operation<"/repos/{owner}/{repo}/issues/{issue_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-issue-comments
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}/comments": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-issue-events
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}/events": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-labels-for-an-issue
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}/labels": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/labels", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-an-issue
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}/reactions": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-timeline-events-for-an-issue
     */
    "GET /repos/{owner}/{repo}/issues/{issue_number}/timeline": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/timeline", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-deploy-keys
     */
    "GET /repos/{owner}/{repo}/keys": Operation<"/repos/{owner}/{repo}/keys", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-deploy-key
     */
    "GET /repos/{owner}/{repo}/keys/{key_id}": Operation<"/repos/{owner}/{repo}/keys/{key_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-labels-for-a-repository
     */
    "GET /repos/{owner}/{repo}/labels": Operation<"/repos/{owner}/{repo}/labels", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#get-a-label
     */
    "GET /repos/{owner}/{repo}/labels/{name}": Operation<"/repos/{owner}/{repo}/labels/{name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-languages
     */
    "GET /repos/{owner}/{repo}/languages": Operation<"/repos/{owner}/{repo}/languages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/licenses/#get-the-license-for-a-repository
     */
    "GET /repos/{owner}/{repo}/license": Operation<"/repos/{owner}/{repo}/license", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-milestones
     */
    "GET /repos/{owner}/{repo}/milestones": Operation<"/repos/{owner}/{repo}/milestones", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#get-a-milestone
     */
    "GET /repos/{owner}/{repo}/milestones/{milestone_number}": Operation<"/repos/{owner}/{repo}/milestones/{milestone_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-labels-for-issues-in-a-milestone
     */
    "GET /repos/{owner}/{repo}/milestones/{milestone_number}/labels": Operation<"/repos/{owner}/{repo}/milestones/{milestone_number}/labels", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repository-notifications-for-the-authenticated-user
     */
    "GET /repos/{owner}/{repo}/notifications": Operation<"/repos/{owner}/{repo}/notifications", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-github-pages-site
     */
    "GET /repos/{owner}/{repo}/pages": Operation<"/repos/{owner}/{repo}/pages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-github-pages-builds
     */
    "GET /repos/{owner}/{repo}/pages/builds": Operation<"/repos/{owner}/{repo}/pages/builds", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-latest-pages-build
     */
    "GET /repos/{owner}/{repo}/pages/builds/latest": Operation<"/repos/{owner}/{repo}/pages/builds/latest", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-github-pages-build
     */
    "GET /repos/{owner}/{repo}/pages/builds/{build_id}": Operation<"/repos/{owner}/{repo}/pages/builds/{build_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-dns-health-check-for-github-pages
     */
    "GET /repos/{owner}/{repo}/pages/health": Operation<"/repos/{owner}/{repo}/pages/health", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-repository-projects
     */
    "GET /repos/{owner}/{repo}/projects": Operation<"/repos/{owner}/{repo}/projects", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-pull-requests
     */
    "GET /repos/{owner}/{repo}/pulls": Operation<"/repos/{owner}/{repo}/pulls", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-review-comments-in-a-repository
     */
    "GET /repos/{owner}/{repo}/pulls/comments": Operation<"/repos/{owner}/{repo}/pulls/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#get-a-review-comment-for-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#list-reactions-for-a-pull-request-review-comment
     */
    "GET /repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#get-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-review-comments-on-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/comments": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-commits-on-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/commits": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/commits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-pull-requests-files
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/files": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/files", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#check-if-a-pull-request-has-been-merged
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/merge": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/merge", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-requested-reviewers-for-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-reviews-for-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/reviews": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#get-a-review-for-a-pull-request
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#list-comments-for-a-pull-request-review
     */
    "GET /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/comments": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-repository-readme
     */
    "GET /repos/{owner}/{repo}/readme": Operation<"/repos/{owner}/{repo}/readme", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-repository-directory-readme
     */
    "GET /repos/{owner}/{repo}/readme/{dir}": Operation<"/repos/{owner}/{repo}/readme/{dir}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-releases
     */
    "GET /repos/{owner}/{repo}/releases": Operation<"/repos/{owner}/{repo}/releases", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-release-asset
     */
    "GET /repos/{owner}/{repo}/releases/assets/{asset_id}": Operation<"/repos/{owner}/{repo}/releases/assets/{asset_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-latest-release
     */
    "GET /repos/{owner}/{repo}/releases/latest": Operation<"/repos/{owner}/{repo}/releases/latest", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-release-by-tag-name
     */
    "GET /repos/{owner}/{repo}/releases/tags/{tag}": Operation<"/repos/{owner}/{repo}/releases/tags/{tag}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-a-release
     */
    "GET /repos/{owner}/{repo}/releases/{release_id}": Operation<"/repos/{owner}/{repo}/releases/{release_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-release-assets
     */
    "GET /repos/{owner}/{repo}/releases/{release_id}/assets": Operation<"/repos/{owner}/{repo}/releases/{release_id}/assets", "get">;
    /**
     * @see https://docs.github.com/rest/reference/secret-scanning#list-secret-scanning-alerts-for-a-repository
     */
    "GET /repos/{owner}/{repo}/secret-scanning/alerts": Operation<"/repos/{owner}/{repo}/secret-scanning/alerts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/secret-scanning#get-a-secret-scanning-alert
     */
    "GET /repos/{owner}/{repo}/secret-scanning/alerts/{alert_number}": Operation<"/repos/{owner}/{repo}/secret-scanning/alerts/{alert_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-stargazers
     */
    "GET /repos/{owner}/{repo}/stargazers": Operation<"/repos/{owner}/{repo}/stargazers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-weekly-commit-activity
     */
    "GET /repos/{owner}/{repo}/stats/code_frequency": Operation<"/repos/{owner}/{repo}/stats/code_frequency", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-last-year-of-commit-activity
     */
    "GET /repos/{owner}/{repo}/stats/commit_activity": Operation<"/repos/{owner}/{repo}/stats/commit_activity", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-all-contributor-commit-activity
     */
    "GET /repos/{owner}/{repo}/stats/contributors": Operation<"/repos/{owner}/{repo}/stats/contributors", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-weekly-commit-count
     */
    "GET /repos/{owner}/{repo}/stats/participation": Operation<"/repos/{owner}/{repo}/stats/participation", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-the-hourly-commit-count-for-each-day
     */
    "GET /repos/{owner}/{repo}/stats/punch_card": Operation<"/repos/{owner}/{repo}/stats/punch_card", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-watchers
     */
    "GET /repos/{owner}/{repo}/subscribers": Operation<"/repos/{owner}/{repo}/subscribers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#get-a-repository-subscription
     */
    "GET /repos/{owner}/{repo}/subscription": Operation<"/repos/{owner}/{repo}/subscription", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-tags
     */
    "GET /repos/{owner}/{repo}/tags": Operation<"/repos/{owner}/{repo}/tags", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#download-a-repository-archive
     */
    "GET /repos/{owner}/{repo}/tarball/{ref}": Operation<"/repos/{owner}/{repo}/tarball/{ref}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-teams
     */
    "GET /repos/{owner}/{repo}/teams": Operation<"/repos/{owner}/{repo}/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-all-repository-topics
     */
    "GET /repos/{owner}/{repo}/topics": Operation<"/repos/{owner}/{repo}/topics", "get", "mercy">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-repository-clones
     */
    "GET /repos/{owner}/{repo}/traffic/clones": Operation<"/repos/{owner}/{repo}/traffic/clones", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-top-referral-paths
     */
    "GET /repos/{owner}/{repo}/traffic/popular/paths": Operation<"/repos/{owner}/{repo}/traffic/popular/paths", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-top-referral-sources
     */
    "GET /repos/{owner}/{repo}/traffic/popular/referrers": Operation<"/repos/{owner}/{repo}/traffic/popular/referrers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#get-page-views
     */
    "GET /repos/{owner}/{repo}/traffic/views": Operation<"/repos/{owner}/{repo}/traffic/views", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#check-if-vulnerability-alerts-are-enabled-for-a-repository
     */
    "GET /repos/{owner}/{repo}/vulnerability-alerts": Operation<"/repos/{owner}/{repo}/vulnerability-alerts", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#download-a-repository-archive
     */
    "GET /repos/{owner}/{repo}/zipball/{ref}": Operation<"/repos/{owner}/{repo}/zipball/{ref}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-public-repositories
     */
    "GET /repositories": Operation<"/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#list-environment-secrets
     */
    "GET /repositories/{repository_id}/environments/{environment_name}/secrets": Operation<"/repositories/{repository_id}/environments/{environment_name}/secrets", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-an-environment-public-key
     */
    "GET /repositories/{repository_id}/environments/{environment_name}/secrets/public-key": Operation<"/repositories/{repository_id}/environments/{environment_name}/secrets/public-key", "get">;
    /**
     * @see https://docs.github.com/rest/reference/actions#get-an-environment-secret
     */
    "GET /repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}": Operation<"/repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-provisioned-scim-groups-for-an-enterprise
     */
    "GET /scim/v2/enterprises/{enterprise}/Groups": Operation<"/scim/v2/enterprises/{enterprise}/Groups", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-scim-provisioning-information-for-an-enterprise-group
     */
    "GET /scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}": Operation<"/scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#list-scim-provisioned-identities-for-an-enterprise
     */
    "GET /scim/v2/enterprises/{enterprise}/Users": Operation<"/scim/v2/enterprises/{enterprise}/Users", "get">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#get-scim-provisioning-information-for-an-enterprise-user
     */
    "GET /scim/v2/enterprises/{enterprise}/Users/{scim_user_id}": Operation<"/scim/v2/enterprises/{enterprise}/Users/{scim_user_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/scim#list-scim-provisioned-identities
     */
    "GET /scim/v2/organizations/{org}/Users": Operation<"/scim/v2/organizations/{org}/Users", "get">;
    /**
     * @see https://docs.github.com/rest/reference/scim#get-scim-provisioning-information-for-a-user
     */
    "GET /scim/v2/organizations/{org}/Users/{scim_user_id}": Operation<"/scim/v2/organizations/{org}/Users/{scim_user_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-code
     */
    "GET /search/code": Operation<"/search/code", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-commits
     */
    "GET /search/commits": Operation<"/search/commits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-issues-and-pull-requests
     */
    "GET /search/issues": Operation<"/search/issues", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-labels
     */
    "GET /search/labels": Operation<"/search/labels", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-repositories
     */
    "GET /search/repositories": Operation<"/search/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-topics
     */
    "GET /search/topics": Operation<"/search/topics", "get", "mercy">;
    /**
     * @see https://docs.github.com/rest/reference/search#search-users
     */
    "GET /search/users": Operation<"/search/users", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#get-a-team-legacy
     */
    "GET /teams/{team_id}": Operation<"/teams/{team_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-discussions-legacy
     */
    "GET /teams/{team_id}/discussions": Operation<"/teams/{team_id}/discussions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-a-discussion-legacy
     */
    "GET /teams/{team_id}/discussions/{discussion_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-discussion-comments-legacy
     */
    "GET /teams/{team_id}/discussions/{discussion_number}/comments": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-a-discussion-comment-legacy
     */
    "GET /teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#list-reactions-for-a-team-discussion-comment-legacy
     */
    "GET /teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}/reactions": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#list-reactions-for-a-team-discussion-legacy
     */
    "GET /teams/{team_id}/discussions/{discussion_number}/reactions": Operation<"/teams/{team_id}/discussions/{discussion_number}/reactions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-pending-team-invitations-legacy
     */
    "GET /teams/{team_id}/invitations": Operation<"/teams/{team_id}/invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-team-members-legacy
     */
    "GET /teams/{team_id}/members": Operation<"/teams/{team_id}/members", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-team-member-legacy
     */
    "GET /teams/{team_id}/members/{username}": Operation<"/teams/{team_id}/members/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#get-team-membership-for-a-user-legacy
     */
    "GET /teams/{team_id}/memberships/{username}": Operation<"/teams/{team_id}/memberships/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#list-team-projects-legacy
     */
    "GET /teams/{team_id}/projects": Operation<"/teams/{team_id}/projects", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#check-team-permissions-for-a-project-legacy
     */
    "GET /teams/{team_id}/projects/{project_id}": Operation<"/teams/{team_id}/projects/{project_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#list-team-repositories-legacy
     */
    "GET /teams/{team_id}/repos": Operation<"/teams/{team_id}/repos", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#check-team-permissions-for-a-repository-legacy
     */
    "GET /teams/{team_id}/repos/{owner}/{repo}": Operation<"/teams/{team_id}/repos/{owner}/{repo}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-idp-groups-for-a-team-legacy
     */
    "GET /teams/{team_id}/team-sync/group-mappings": Operation<"/teams/{team_id}/team-sync/group-mappings", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#list-child-teams-legacy
     */
    "GET /teams/{team_id}/teams": Operation<"/teams/{team_id}/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#get-the-authenticated-user
     */
    "GET /user": Operation<"/user", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-users-blocked-by-the-authenticated-user
     */
    "GET /user/blocks": Operation<"/user/blocks", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#check-if-a-user-is-blocked-by-the-authenticated-user
     */
    "GET /user/blocks/{username}": Operation<"/user/blocks/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-email-addresses-for-the-authenticated-user
     */
    "GET /user/emails": Operation<"/user/emails", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-followers-of-the-authenticated-user
     */
    "GET /user/followers": Operation<"/user/followers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-the-people-the-authenticated-user-follows
     */
    "GET /user/following": Operation<"/user/following", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#check-if-a-person-is-followed-by-the-authenticated-user
     */
    "GET /user/following/{username}": Operation<"/user/following/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-gpg-keys-for-the-authenticated-user
     */
    "GET /user/gpg_keys": Operation<"/user/gpg_keys", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#get-a-gpg-key-for-the-authenticated-user
     */
    "GET /user/gpg_keys/{gpg_key_id}": Operation<"/user/gpg_keys/{gpg_key_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-app-installations-accessible-to-the-user-access-token
     */
    "GET /user/installations": Operation<"/user/installations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-repositories-accessible-to-the-user-access-token
     */
    "GET /user/installations/{installation_id}/repositories": Operation<"/user/installations/{installation_id}/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#get-interaction-restrictions-for-your-public-repositories
     */
    "GET /user/interaction-limits": Operation<"/user/interaction-limits", "get">;
    /**
     * @see https://docs.github.com/rest/reference/issues#list-user-account-issues-assigned-to-the-authenticated-user
     */
    "GET /user/issues": Operation<"/user/issues", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-public-ssh-keys-for-the-authenticated-user
     */
    "GET /user/keys": Operation<"/user/keys", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#get-a-public-ssh-key-for-the-authenticated-user
     */
    "GET /user/keys/{key_id}": Operation<"/user/keys/{key_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-subscriptions-for-the-authenticated-user
     */
    "GET /user/marketplace_purchases": Operation<"/user/marketplace_purchases", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#list-subscriptions-for-the-authenticated-user-stubbed
     */
    "GET /user/marketplace_purchases/stubbed": Operation<"/user/marketplace_purchases/stubbed", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organization-memberships-for-the-authenticated-user
     */
    "GET /user/memberships/orgs": Operation<"/user/memberships/orgs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#get-an-organization-membership-for-the-authenticated-user
     */
    "GET /user/memberships/orgs/{org}": Operation<"/user/memberships/orgs/{org}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#list-user-migrations
     */
    "GET /user/migrations": Operation<"/user/migrations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#get-a-user-migration-status
     */
    "GET /user/migrations/{migration_id}": Operation<"/user/migrations/{migration_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#download-a-user-migration-archive
     */
    "GET /user/migrations/{migration_id}/archive": Operation<"/user/migrations/{migration_id}/archive", "get">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#list-repositories-for-a-user-migration
     */
    "GET /user/migrations/{migration_id}/repositories": Operation<"/user/migrations/{migration_id}/repositories", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organizations-for-the-authenticated-user
     */
    "GET /user/orgs": Operation<"/user/orgs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#list-packages-for-the-authenticated-user
     */
    "GET /user/packages": Operation<"/user/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-for-the-authenticated-user
     */
    "GET /user/packages/{package_type}/{package_name}": Operation<"/user/packages/{package_type}/{package_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-all-package-versions-for-a-package-owned-by-the-authenticated-user
     */
    "GET /user/packages/{package_type}/{package_name}/versions": Operation<"/user/packages/{package_type}/{package_name}/versions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-version-for-the-authenticated-user
     */
    "GET /user/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/user/packages/{package_type}/{package_name}/versions/{package_version_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-public-email-addresses-for-the-authenticated-user
     */
    "GET /user/public_emails": Operation<"/user/public_emails", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repositories-for-the-authenticated-user
     */
    "GET /user/repos": Operation<"/user/repos", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repository-invitations-for-the-authenticated-user
     */
    "GET /user/repository_invitations": Operation<"/user/repository_invitations", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repositories-starred-by-the-authenticated-user
     */
    "GET /user/starred": Operation<"/user/starred", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#check-if-a-repository-is-starred-by-the-authenticated-user
     */
    "GET /user/starred/{owner}/{repo}": Operation<"/user/starred/{owner}/{repo}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repositories-watched-by-the-authenticated-user
     */
    "GET /user/subscriptions": Operation<"/user/subscriptions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/teams#list-teams-for-the-authenticated-user
     */
    "GET /user/teams": Operation<"/user/teams", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-users
     */
    "GET /users": Operation<"/users", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#get-a-user
     */
    "GET /users/{username}": Operation<"/users/{username}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-events-for-the-authenticated-user
     */
    "GET /users/{username}/events": Operation<"/users/{username}/events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-organization-events-for-the-authenticated-user
     */
    "GET /users/{username}/events/orgs/{org}": Operation<"/users/{username}/events/orgs/{org}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-public-events-for-a-user
     */
    "GET /users/{username}/events/public": Operation<"/users/{username}/events/public", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-followers-of-a-user
     */
    "GET /users/{username}/followers": Operation<"/users/{username}/followers", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-the-people-a-user-follows
     */
    "GET /users/{username}/following": Operation<"/users/{username}/following", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#check-if-a-user-follows-another-user
     */
    "GET /users/{username}/following/{target_user}": Operation<"/users/{username}/following/{target_user}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/gists#list-gists-for-a-user
     */
    "GET /users/{username}/gists": Operation<"/users/{username}/gists", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-gpg-keys-for-a-user
     */
    "GET /users/{username}/gpg_keys": Operation<"/users/{username}/gpg_keys", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#get-contextual-information-for-a-user
     */
    "GET /users/{username}/hovercard": Operation<"/users/{username}/hovercard", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#get-a-user-installation-for-the-authenticated-app
     */
    "GET /users/{username}/installation": Operation<"/users/{username}/installation", "get">;
    /**
     * @see https://docs.github.com/rest/reference/users#list-public-keys-for-a-user
     */
    "GET /users/{username}/keys": Operation<"/users/{username}/keys", "get">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#list-organizations-for-a-user
     */
    "GET /users/{username}/orgs": Operation<"/users/{username}/orgs", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#list-packages-for-user
     */
    "GET /users/{username}/packages": Operation<"/users/{username}/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-for-a-user
     */
    "GET /users/{username}/packages/{package_type}/{package_name}": Operation<"/users/{username}/packages/{package_type}/{package_name}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-all-package-versions-for-a-package-owned-by-a-user
     */
    "GET /users/{username}/packages/{package_type}/{package_name}/versions": Operation<"/users/{username}/packages/{package_type}/{package_name}/versions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/packages#get-a-package-version-for-a-user
     */
    "GET /users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}": Operation<"/users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}", "get">;
    /**
     * @see https://docs.github.com/rest/reference/projects#list-user-projects
     */
    "GET /users/{username}/projects": Operation<"/users/{username}/projects", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-events-received-by-the-authenticated-user
     */
    "GET /users/{username}/received_events": Operation<"/users/{username}/received_events", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-public-events-received-by-a-user
     */
    "GET /users/{username}/received_events/public": Operation<"/users/{username}/received_events/public", "get">;
    /**
     * @see https://docs.github.com/rest/reference/repos#list-repositories-for-a-user
     */
    "GET /users/{username}/repos": Operation<"/users/{username}/repos", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-actions-billing-for-a-user
     */
    "GET /users/{username}/settings/billing/actions": Operation<"/users/{username}/settings/billing/actions", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-github-packages-billing-for-a-user
     */
    "GET /users/{username}/settings/billing/packages": Operation<"/users/{username}/settings/billing/packages", "get">;
    /**
     * @see https://docs.github.com/rest/reference/billing#get-shared-storage-billing-for-a-user
     */
    "GET /users/{username}/settings/billing/shared-storage": Operation<"/users/{username}/settings/billing/shared-storage", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repositories-starred-by-a-user
     */
    "GET /users/{username}/starred": Operation<"/users/{username}/starred", "get">;
    /**
     * @see https://docs.github.com/rest/reference/activity#list-repositories-watched-by-a-user
     */
    "GET /users/{username}/subscriptions": Operation<"/users/{username}/subscriptions", "get">;
    /**
     * @see
     */
    "GET /zen": Operation<"/zen", "get">;
    /**
     * @see https://docs.github.com/rest/reference/apps#update-a-webhook-configuration-for-an-app
     */
    "PATCH /app/hook/config": Operation<"/app/hook/config", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/apps#reset-a-token
     */
    "PATCH /applications/{client_id}/token": Operation<"/applications/{client_id}/token", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#update-an-existing-authorization
     */
    "PATCH /authorizations/{authorization_id}": Operation<"/authorizations/{authorization_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#update-a-self-hosted-runner-group-for-an-enterprise
     */
    "PATCH /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/gists/#update-a-gist
     */
    "PATCH /gists/{gist_id}": Operation<"/gists/{gist_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/gists#update-a-gist-comment
     */
    "PATCH /gists/{gist_id}/comments/{comment_id}": Operation<"/gists/{gist_id}/comments/{comment_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/activity#mark-a-thread-as-read
     */
    "PATCH /notifications/threads/{thread_id}": Operation<"/notifications/threads/{thread_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/orgs/#update-an-organization
     */
    "PATCH /orgs/{org}": Operation<"/orgs/{org}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/actions#update-a-self-hosted-runner-group-for-an-organization
     */
    "PATCH /orgs/{org}/actions/runner-groups/{runner_group_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#update-an-organization-webhook
     */
    "PATCH /orgs/{org}/hooks/{hook_id}": Operation<"/orgs/{org}/hooks/{hook_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#update-a-webhook-configuration-for-an-organization
     */
    "PATCH /orgs/{org}/hooks/{hook_id}/config": Operation<"/orgs/{org}/hooks/{hook_id}/config", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#update-a-team
     */
    "PATCH /orgs/{org}/teams/{team_slug}": Operation<"/orgs/{org}/teams/{team_slug}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#update-a-discussion
     */
    "PATCH /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#update-a-discussion-comment
     */
    "PATCH /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-or-update-idp-group-connections
     */
    "PATCH /orgs/{org}/teams/{team_slug}/team-sync/group-mappings": Operation<"/orgs/{org}/teams/{team_slug}/team-sync/group-mappings", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/projects#update-a-project-card
     */
    "PATCH /projects/columns/cards/{card_id}": Operation<"/projects/columns/cards/{card_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/projects#update-a-project-column
     */
    "PATCH /projects/columns/{column_id}": Operation<"/projects/columns/{column_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/projects#update-a-project
     */
    "PATCH /projects/{project_id}": Operation<"/projects/{project_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos/#update-a-repository
     */
    "PATCH /repos/{owner}/{repo}": Operation<"/repos/{owner}/{repo}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-pull-request-review-protection
     */
    "PATCH /repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_pull_request_reviews", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-status-check-potection
     */
    "PATCH /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/checks#update-a-check-run
     */
    "PATCH /repos/{owner}/{repo}/check-runs/{check_run_id}": Operation<"/repos/{owner}/{repo}/check-runs/{check_run_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/checks#update-repository-preferences-for-check-suites
     */
    "PATCH /repos/{owner}/{repo}/check-suites/preferences": Operation<"/repos/{owner}/{repo}/check-suites/preferences", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#update-a-code-scanning-alert
     */
    "PATCH /repos/{owner}/{repo}/code-scanning/alerts/{alert_number}": Operation<"/repos/{owner}/{repo}/code-scanning/alerts/{alert_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-commit-comment
     */
    "PATCH /repos/{owner}/{repo}/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/comments/{comment_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/git#update-a-reference
     */
    "PATCH /repos/{owner}/{repo}/git/refs/{ref}": Operation<"/repos/{owner}/{repo}/git/refs/{ref}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-repository-webhook
     */
    "PATCH /repos/{owner}/{repo}/hooks/{hook_id}": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-webhook-configuration-for-a-repository
     */
    "PATCH /repos/{owner}/{repo}/hooks/{hook_id}/config": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/config", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#update-an-import
     */
    "PATCH /repos/{owner}/{repo}/import": Operation<"/repos/{owner}/{repo}/import", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#map-a-commit-author
     */
    "PATCH /repos/{owner}/{repo}/import/authors/{author_id}": Operation<"/repos/{owner}/{repo}/import/authors/{author_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#update-git-lfs-preference
     */
    "PATCH /repos/{owner}/{repo}/import/lfs": Operation<"/repos/{owner}/{repo}/import/lfs", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-repository-invitation
     */
    "PATCH /repos/{owner}/{repo}/invitations/{invitation_id}": Operation<"/repos/{owner}/{repo}/invitations/{invitation_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/issues#update-an-issue-comment
     */
    "PATCH /repos/{owner}/{repo}/issues/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/issues/#update-an-issue
     */
    "PATCH /repos/{owner}/{repo}/issues/{issue_number}": Operation<"/repos/{owner}/{repo}/issues/{issue_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/issues#update-a-label
     */
    "PATCH /repos/{owner}/{repo}/labels/{name}": Operation<"/repos/{owner}/{repo}/labels/{name}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/issues#update-a-milestone
     */
    "PATCH /repos/{owner}/{repo}/milestones/{milestone_number}": Operation<"/repos/{owner}/{repo}/milestones/{milestone_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#update-a-review-comment-for-a-pull-request
     */
    "PATCH /repos/{owner}/{repo}/pulls/comments/{comment_id}": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/pulls/#update-a-pull-request
     */
    "PATCH /repos/{owner}/{repo}/pulls/{pull_number}": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-release-asset
     */
    "PATCH /repos/{owner}/{repo}/releases/assets/{asset_id}": Operation<"/repos/{owner}/{repo}/releases/assets/{asset_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-a-release
     */
    "PATCH /repos/{owner}/{repo}/releases/{release_id}": Operation<"/repos/{owner}/{repo}/releases/{release_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/secret-scanning#update-a-secret-scanning-alert
     */
    "PATCH /repos/{owner}/{repo}/secret-scanning/alerts/{alert_number}": Operation<"/repos/{owner}/{repo}/secret-scanning/alerts/{alert_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#update-an-attribute-for-a-scim-enterprise-group
     */
    "PATCH /scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}": Operation<"/scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#update-an-attribute-for-a-scim-enterprise-user
     */
    "PATCH /scim/v2/enterprises/{enterprise}/Users/{scim_user_id}": Operation<"/scim/v2/enterprises/{enterprise}/Users/{scim_user_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/scim#update-an-attribute-for-a-scim-user
     */
    "PATCH /scim/v2/organizations/{org}/Users/{scim_user_id}": Operation<"/scim/v2/organizations/{org}/Users/{scim_user_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#update-a-team-legacy
     */
    "PATCH /teams/{team_id}": Operation<"/teams/{team_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#update-a-discussion-legacy
     */
    "PATCH /teams/{team_id}/discussions/{discussion_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#update-a-discussion-comment-legacy
     */
    "PATCH /teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-or-update-idp-group-connections-legacy
     */
    "PATCH /teams/{team_id}/team-sync/group-mappings": Operation<"/teams/{team_id}/team-sync/group-mappings", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/users/#update-the-authenticated-user
     */
    "PATCH /user": Operation<"/user", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/users#set-primary-email-visibility-for-the-authenticated-user
     */
    "PATCH /user/email/visibility": Operation<"/user/email/visibility", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#update-an-organization-membership-for-the-authenticated-user
     */
    "PATCH /user/memberships/orgs/{org}": Operation<"/user/memberships/orgs/{org}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/repos#accept-a-repository-invitation
     */
    "PATCH /user/repository_invitations/{invitation_id}": Operation<"/user/repository_invitations/{invitation_id}", "patch">;
    /**
     * @see https://docs.github.com/rest/reference/apps#create-a-github-app-from-a-manifest
     */
    "POST /app-manifests/{code}/conversions": Operation<"/app-manifests/{code}/conversions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#redeliver-a-delivery-for-an-app-webhook
     */
    "POST /app/hook/deliveries/{delivery_id}/attempts": Operation<"/app/hook/deliveries/{delivery_id}/attempts", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps/#create-an-installation-access-token-for-an-app
     */
    "POST /app/installations/{installation_id}/access_tokens": Operation<"/app/installations/{installation_id}/access_tokens", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#check-a-token
     */
    "POST /applications/{client_id}/token": Operation<"/applications/{client_id}/token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#create-a-scoped-access-token
     */
    "POST /applications/{client_id}/token/scoped": Operation<"/applications/{client_id}/token/scoped", "post">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#create-a-new-authorization
     */
    "POST /authorizations": Operation<"/authorizations", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#create-a-content-attachment
     */
    "POST /content_references/{content_reference_id}/attachments": Operation<"/content_references/{content_reference_id}/attachments", "post", "corsair">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#create-self-hosted-runner-group-for-an-enterprise
     */
    "POST /enterprises/{enterprise}/actions/runner-groups": Operation<"/enterprises/{enterprise}/actions/runner-groups", "post">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#create-a-registration-token-for-an-enterprise
     */
    "POST /enterprises/{enterprise}/actions/runners/registration-token": Operation<"/enterprises/{enterprise}/actions/runners/registration-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#create-a-remove-token-for-an-enterprise
     */
    "POST /enterprises/{enterprise}/actions/runners/remove-token": Operation<"/enterprises/{enterprise}/actions/runners/remove-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/gists#create-a-gist
     */
    "POST /gists": Operation<"/gists", "post">;
    /**
     * @see https://docs.github.com/rest/reference/gists#create-a-gist-comment
     */
    "POST /gists/{gist_id}/comments": Operation<"/gists/{gist_id}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/gists#fork-a-gist
     */
    "POST /gists/{gist_id}/forks": Operation<"/gists/{gist_id}/forks", "post">;
    /**
     * @see https://docs.github.com/rest/reference/markdown#render-a-markdown-document
     */
    "POST /markdown": Operation<"/markdown", "post">;
    /**
     * @see https://docs.github.com/rest/reference/markdown#render-a-markdown-document-in-raw-mode
     */
    "POST /markdown/raw": Operation<"/markdown/raw", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-self-hosted-runner-group-for-an-organization
     */
    "POST /orgs/{org}/actions/runner-groups": Operation<"/orgs/{org}/actions/runner-groups", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-registration-token-for-an-organization
     */
    "POST /orgs/{org}/actions/runners/registration-token": Operation<"/orgs/{org}/actions/runners/registration-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-remove-token-for-an-organization
     */
    "POST /orgs/{org}/actions/runners/remove-token": Operation<"/orgs/{org}/actions/runners/remove-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#create-an-organization-webhook
     */
    "POST /orgs/{org}/hooks": Operation<"/orgs/{org}/hooks", "post">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#redeliver-a-delivery-for-an-organization-webhook
     */
    "POST /orgs/{org}/hooks/{hook_id}/deliveries/{delivery_id}/attempts": Operation<"/orgs/{org}/hooks/{hook_id}/deliveries/{delivery_id}/attempts", "post">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#ping-an-organization-webhook
     */
    "POST /orgs/{org}/hooks/{hook_id}/pings": Operation<"/orgs/{org}/hooks/{hook_id}/pings", "post">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#create-an-organization-invitation
     */
    "POST /orgs/{org}/invitations": Operation<"/orgs/{org}/invitations", "post">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#start-an-organization-migration
     */
    "POST /orgs/{org}/migrations": Operation<"/orgs/{org}/migrations", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-for-an-organization
     */
    "POST /orgs/{org}/packages/{package_type}/{package_name}/restore{?token}": Operation<"/orgs/{org}/packages/{package_type}/{package_name}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-version-for-an-organization
     */
    "POST /orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}/restore": Operation<"/orgs/{org}/packages/{package_type}/{package_name}/versions/{package_version_id}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#create-an-organization-project
     */
    "POST /orgs/{org}/projects": Operation<"/orgs/{org}/projects", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-an-organization-repository
     */
    "POST /orgs/{org}/repos": Operation<"/orgs/{org}/repos", "post">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-a-team
     */
    "POST /orgs/{org}/teams": Operation<"/orgs/{org}/teams", "post">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-a-discussion
     */
    "POST /orgs/{org}/teams/{team_slug}/discussions": Operation<"/orgs/{org}/teams/{team_slug}/discussions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-a-discussion-comment
     */
    "POST /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-a-team-discussion-comment
     */
    "POST /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/comments/{comment_number}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-a-team-discussion
     */
    "POST /orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions": Operation<"/orgs/{org}/teams/{team_slug}/discussions/{discussion_number}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#move-a-project-card
     */
    "POST /projects/columns/cards/{card_id}/moves": Operation<"/projects/columns/cards/{card_id}/moves", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#create-a-project-card
     */
    "POST /projects/columns/{column_id}/cards": Operation<"/projects/columns/{column_id}/cards", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#move-a-project-column
     */
    "POST /projects/columns/{column_id}/moves": Operation<"/projects/columns/{column_id}/moves", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#create-a-project-column
     */
    "POST /projects/{project_id}/columns": Operation<"/projects/{project_id}/columns", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-registration-token-for-a-repository
     */
    "POST /repos/{owner}/{repo}/actions/runners/registration-token": Operation<"/repos/{owner}/{repo}/actions/runners/registration-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-remove-token-for-a-repository
     */
    "POST /repos/{owner}/{repo}/actions/runners/remove-token": Operation<"/repos/{owner}/{repo}/actions/runners/remove-token", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#approve-a-workflow-run-for-a-fork-pull-request
     */
    "POST /repos/{owner}/{repo}/actions/runs/{run_id}/approve": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/approve", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#cancel-a-workflow-run
     */
    "POST /repos/{owner}/{repo}/actions/runs/{run_id}/cancel": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/cancel", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#review-pending-deployments-for-a-workflow-run
     */
    "POST /repos/{owner}/{repo}/actions/runs/{run_id}/pending_deployments": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/pending_deployments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#re-run-a-workflow
     */
    "POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun": Operation<"/repos/{owner}/{repo}/actions/runs/{run_id}/rerun", "post">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-a-workflow-dispatch-event
     */
    "POST /repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches", "post">;
    /**
     * @see https://docs.github.com/v3/repos#create-an-autolink
     */
    "POST /repos/{owner}/{repo}/autolinks": Operation<"/repos/{owner}/{repo}/autolinks", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#set-admin-branch-protection
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/enforce_admins", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-commit-signature-protection
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/required_signatures": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_signatures", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#add-status-check-contexts
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#add-app-access-restrictions
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#add-team-access-restrictions
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#add-user-access-restrictions
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#rename-a-branch
     */
    "POST /repos/{owner}/{repo}/branches/{branch}/rename": Operation<"/repos/{owner}/{repo}/branches/{branch}/rename", "post">;
    /**
     * @see https://docs.github.com/rest/reference/checks#create-a-check-run
     */
    "POST /repos/{owner}/{repo}/check-runs": Operation<"/repos/{owner}/{repo}/check-runs", "post">;
    /**
     * @see https://docs.github.com/rest/reference/checks#rerequest-a-check-run
     */
    "POST /repos/{owner}/{repo}/check-runs/{check_run_id}/rerequest": Operation<"/repos/{owner}/{repo}/check-runs/{check_run_id}/rerequest", "post">;
    /**
     * @see https://docs.github.com/rest/reference/checks#create-a-check-suite
     */
    "POST /repos/{owner}/{repo}/check-suites": Operation<"/repos/{owner}/{repo}/check-suites", "post">;
    /**
     * @see https://docs.github.com/rest/reference/checks#rerequest-a-check-suite
     */
    "POST /repos/{owner}/{repo}/check-suites/{check_suite_id}/rerequest": Operation<"/repos/{owner}/{repo}/check-suites/{check_suite_id}/rerequest", "post">;
    /**
     * @see https://docs.github.com/rest/reference/code-scanning#upload-a-sarif-file
     */
    "POST /repos/{owner}/{repo}/code-scanning/sarifs": Operation<"/repos/{owner}/{repo}/code-scanning/sarifs", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-a-commit-comment
     */
    "POST /repos/{owner}/{repo}/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/comments/{comment_id}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-commit-comment
     */
    "POST /repos/{owner}/{repo}/commits/{commit_sha}/comments": Operation<"/repos/{owner}/{repo}/commits/{commit_sha}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#create-a-content-attachment
     */
    "POST /repos/{owner}/{repo}/content_references/{content_reference_id}/attachments": Operation<"/repos/{owner}/{repo}/content_references/{content_reference_id}/attachments", "post", "corsair">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-deployment
     */
    "POST /repos/{owner}/{repo}/deployments": Operation<"/repos/{owner}/{repo}/deployments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-deployment-status
     */
    "POST /repos/{owner}/{repo}/deployments/{deployment_id}/statuses": Operation<"/repos/{owner}/{repo}/deployments/{deployment_id}/statuses", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-repository-dispatch-event
     */
    "POST /repos/{owner}/{repo}/dispatches": Operation<"/repos/{owner}/{repo}/dispatches", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-fork
     */
    "POST /repos/{owner}/{repo}/forks": Operation<"/repos/{owner}/{repo}/forks", "post">;
    /**
     * @see https://docs.github.com/rest/reference/git#create-a-blob
     */
    "POST /repos/{owner}/{repo}/git/blobs": Operation<"/repos/{owner}/{repo}/git/blobs", "post">;
    /**
     * @see https://docs.github.com/rest/reference/git#create-a-commit
     */
    "POST /repos/{owner}/{repo}/git/commits": Operation<"/repos/{owner}/{repo}/git/commits", "post">;
    /**
     * @see https://docs.github.com/rest/reference/git#create-a-reference
     */
    "POST /repos/{owner}/{repo}/git/refs": Operation<"/repos/{owner}/{repo}/git/refs", "post">;
    /**
     * @see https://docs.github.com/rest/reference/git#create-a-tag-object
     */
    "POST /repos/{owner}/{repo}/git/tags": Operation<"/repos/{owner}/{repo}/git/tags", "post">;
    /**
     * @see https://docs.github.com/rest/reference/git#create-a-tree
     */
    "POST /repos/{owner}/{repo}/git/trees": Operation<"/repos/{owner}/{repo}/git/trees", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-repository-webhook
     */
    "POST /repos/{owner}/{repo}/hooks": Operation<"/repos/{owner}/{repo}/hooks", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#redeliver-a-delivery-for-a-repository-webhook
     */
    "POST /repos/{owner}/{repo}/hooks/{hook_id}/deliveries/{delivery_id}/attempts": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/deliveries/{delivery_id}/attempts", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#ping-a-repository-webhook
     */
    "POST /repos/{owner}/{repo}/hooks/{hook_id}/pings": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/pings", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#test-the-push-repository-webhook
     */
    "POST /repos/{owner}/{repo}/hooks/{hook_id}/tests": Operation<"/repos/{owner}/{repo}/hooks/{hook_id}/tests", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#create-an-issue
     */
    "POST /repos/{owner}/{repo}/issues": Operation<"/repos/{owner}/{repo}/issues", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-an-issue-comment
     */
    "POST /repos/{owner}/{repo}/issues/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#add-assignees-to-an-issue
     */
    "POST /repos/{owner}/{repo}/issues/{issue_number}/assignees": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/assignees", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#create-an-issue-comment
     */
    "POST /repos/{owner}/{repo}/issues/{issue_number}/comments": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#add-labels-to-an-issue
     */
    "POST /repos/{owner}/{repo}/issues/{issue_number}/labels": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/labels", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-an-issue
     */
    "POST /repos/{owner}/{repo}/issues/{issue_number}/reactions": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-deploy-key
     */
    "POST /repos/{owner}/{repo}/keys": Operation<"/repos/{owner}/{repo}/keys", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#create-a-label
     */
    "POST /repos/{owner}/{repo}/labels": Operation<"/repos/{owner}/{repo}/labels", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#sync-a-fork-branch-with-the-upstream-repository
     */
    "POST /repos/{owner}/{repo}/merge-upstream": Operation<"/repos/{owner}/{repo}/merge-upstream", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#merge-a-branch
     */
    "POST /repos/{owner}/{repo}/merges": Operation<"/repos/{owner}/{repo}/merges", "post">;
    /**
     * @see https://docs.github.com/rest/reference/issues#create-a-milestone
     */
    "POST /repos/{owner}/{repo}/milestones": Operation<"/repos/{owner}/{repo}/milestones", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-github-pages-site
     */
    "POST /repos/{owner}/{repo}/pages": Operation<"/repos/{owner}/{repo}/pages", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#request-a-github-pages-build
     */
    "POST /repos/{owner}/{repo}/pages/builds": Operation<"/repos/{owner}/{repo}/pages/builds", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#create-a-repository-project
     */
    "POST /repos/{owner}/{repo}/projects": Operation<"/repos/{owner}/{repo}/projects", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#create-a-pull-request
     */
    "POST /repos/{owner}/{repo}/pulls": Operation<"/repos/{owner}/{repo}/pulls", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions#create-reaction-for-a-pull-request-review-comment
     */
    "POST /repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions": Operation<"/repos/{owner}/{repo}/pulls/comments/{comment_id}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#create-a-review-comment-for-a-pull-request
     */
    "POST /repos/{owner}/{repo}/pulls/{pull_number}/comments": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#create-a-reply-for-a-review-comment
     */
    "POST /repos/{owner}/{repo}/pulls/{pull_number}/comments/{comment_id}/replies": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/comments/{comment_id}/replies", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#request-reviewers-for-a-pull-request
     */
    "POST /repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#create-a-review-for-a-pull-request
     */
    "POST /repos/{owner}/{repo}/pulls/{pull_number}/reviews": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews", "post">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#submit-a-review-for-a-pull-request
     */
    "POST /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/events": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/events", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-release
     */
    "POST /repos/{owner}/{repo}/releases": Operation<"/repos/{owner}/{repo}/releases", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#generate-release-notes
     */
    "POST /repos/{owner}/{repo}/releases/generate-notes": Operation<"/repos/{owner}/{repo}/releases/generate-notes", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#create-reaction-for-a-release
     */
    "POST /repos/{owner}/{repo}/releases/{release_id}/reactions": Operation<"/repos/{owner}/{repo}/releases/{release_id}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-commit-status
     */
    "POST /repos/{owner}/{repo}/statuses/{sha}": Operation<"/repos/{owner}/{repo}/statuses/{sha}", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#transfer-a-repository
     */
    "POST /repos/{owner}/{repo}/transfer": Operation<"/repos/{owner}/{repo}/transfer", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-repository-using-a-template
     */
    "POST /repos/{template_owner}/{template_repo}/generate": Operation<"/repos/{template_owner}/{template_repo}/generate", "post">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#provision-a-scim-enterprise-group-and-invite-users
     */
    "POST /scim/v2/enterprises/{enterprise}/Groups": Operation<"/scim/v2/enterprises/{enterprise}/Groups", "post">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#provision-and-invite-a-scim-enterprise-user
     */
    "POST /scim/v2/enterprises/{enterprise}/Users": Operation<"/scim/v2/enterprises/{enterprise}/Users", "post">;
    /**
     * @see https://docs.github.com/rest/reference/scim#provision-and-invite-a-scim-user
     */
    "POST /scim/v2/organizations/{org}/Users": Operation<"/scim/v2/organizations/{org}/Users", "post">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-a-discussion-legacy
     */
    "POST /teams/{team_id}/discussions": Operation<"/teams/{team_id}/discussions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/teams#create-a-discussion-comment-legacy
     */
    "POST /teams/{team_id}/discussions/{discussion_number}/comments": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#create-reaction-for-a-team-discussion-comment-legacy
     */
    "POST /teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}/reactions": Operation<"/teams/{team_id}/discussions/{discussion_number}/comments/{comment_number}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/reactions/#create-reaction-for-a-team-discussion-legacy
     */
    "POST /teams/{team_id}/discussions/{discussion_number}/reactions": Operation<"/teams/{team_id}/discussions/{discussion_number}/reactions", "post">;
    /**
     * @see https://docs.github.com/rest/reference/users#add-an-email-address-for-the-authenticated-user
     */
    "POST /user/emails": Operation<"/user/emails", "post">;
    /**
     * @see https://docs.github.com/rest/reference/users#create-a-gpg-key-for-the-authenticated-user
     */
    "POST /user/gpg_keys": Operation<"/user/gpg_keys", "post">;
    /**
     * @see https://docs.github.com/rest/reference/users#create-a-public-ssh-key-for-the-authenticated-user
     */
    "POST /user/keys": Operation<"/user/keys", "post">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#start-a-user-migration
     */
    "POST /user/migrations": Operation<"/user/migrations", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-for-the-authenticated-user
     */
    "POST /user/packages/{package_type}/{package_name}/restore{?token}": Operation<"/user/packages/{package_type}/{package_name}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-version-for-the-authenticated-user
     */
    "POST /user/packages/{package_type}/{package_name}/versions/{package_version_id}/restore": Operation<"/user/packages/{package_type}/{package_name}/versions/{package_version_id}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/projects#create-a-user-project
     */
    "POST /user/projects": Operation<"/user/projects", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-a-repository-for-the-authenticated-user
     */
    "POST /user/repos": Operation<"/user/repos", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-for-a-user
     */
    "POST /users/{username}/packages/{package_type}/{package_name}/restore{?token}": Operation<"/users/{username}/packages/{package_type}/{package_name}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/packages#restore-a-package-version-for-a-user
     */
    "POST /users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}/restore": Operation<"/users/{username}/packages/{package_type}/{package_name}/versions/{package_version_id}/restore", "post">;
    /**
     * @see https://docs.github.com/rest/reference/repos#upload-a-release-asset
     */
    "POST {origin}/repos/{owner}/{repo}/releases/{release_id}/assets{?name,label}": Operation<"/repos/{owner}/{repo}/releases/{release_id}/assets", "post">;
    /**
     * @see https://docs.github.com/rest/reference/apps#suspend-an-app-installation
     */
    "PUT /app/installations/{installation_id}/suspended": Operation<"/app/installations/{installation_id}/suspended", "put">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#get-or-create-an-authorization-for-a-specific-app
     */
    "PUT /authorizations/clients/{client_id}": Operation<"/authorizations/clients/{client_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/oauth-authorizations#get-or-create-an-authorization-for-a-specific-app-and-fingerprint
     */
    "PUT /authorizations/clients/{client_id}/{fingerprint}": Operation<"/authorizations/clients/{client_id}/{fingerprint}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-github-actions-permissions-for-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/permissions": Operation<"/enterprises/{enterprise}/actions/permissions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-selected-organizations-enabled-for-github-actions-in-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/permissions/organizations": Operation<"/enterprises/{enterprise}/actions/permissions/organizations", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#enable-a-selected-organization-for-github-actions-in-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/permissions/organizations/{org_id}": Operation<"/enterprises/{enterprise}/actions/permissions/organizations/{org_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-allowed-actions-for-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/permissions/selected-actions": Operation<"/enterprises/{enterprise}/actions/permissions/selected-actions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-organization-access-to-a-self-hosted-runner-group-in-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#add-organization-access-to-a-self-hosted-runner-group-in-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations/{org_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/organizations/{org_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-self-hosted-runners-in-a-group-for-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#add-a-self-hosted-runner-to-a-group-for-an-enterprise
     */
    "PUT /enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners/{runner_id}": Operation<"/enterprises/{enterprise}/actions/runner-groups/{runner_group_id}/runners/{runner_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/gists#star-a-gist
     */
    "PUT /gists/{gist_id}/star": Operation<"/gists/{gist_id}/star", "put">;
    /**
     * @see https://docs.github.com/rest/reference/activity#mark-notifications-as-read
     */
    "PUT /notifications": Operation<"/notifications", "put">;
    /**
     * @see https://docs.github.com/rest/reference/activity#set-a-thread-subscription
     */
    "PUT /notifications/threads/{thread_id}/subscription": Operation<"/notifications/threads/{thread_id}/subscription", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-github-actions-permissions-for-an-organization
     */
    "PUT /orgs/{org}/actions/permissions": Operation<"/orgs/{org}/actions/permissions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-selected-repositories-enabled-for-github-actions-in-an-organization
     */
    "PUT /orgs/{org}/actions/permissions/repositories": Operation<"/orgs/{org}/actions/permissions/repositories", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#enable-a-selected-repository-for-github-actions-in-an-organization
     */
    "PUT /orgs/{org}/actions/permissions/repositories/{repository_id}": Operation<"/orgs/{org}/actions/permissions/repositories/{repository_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-allowed-actions-for-an-organization
     */
    "PUT /orgs/{org}/actions/permissions/selected-actions": Operation<"/orgs/{org}/actions/permissions/selected-actions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-repository-access-to-a-self-hosted-runner-group-in-an-organization
     */
    "PUT /orgs/{org}/actions/runner-groups/{runner_group_id}/repositories": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/repositories", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#add-repository-acess-to-a-self-hosted-runner-group-in-an-organization
     */
    "PUT /orgs/{org}/actions/runner-groups/{runner_group_id}/repositories/{repository_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/repositories/{repository_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-self-hosted-runners-in-a-group-for-an-organization
     */
    "PUT /orgs/{org}/actions/runner-groups/{runner_group_id}/runners": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/runners", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#add-a-self-hosted-runner-to-a-group-for-an-organization
     */
    "PUT /orgs/{org}/actions/runner-groups/{runner_group_id}/runners/{runner_id}": Operation<"/orgs/{org}/actions/runner-groups/{runner_group_id}/runners/{runner_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-or-update-an-organization-secret
     */
    "PUT /orgs/{org}/actions/secrets/{secret_name}": Operation<"/orgs/{org}/actions/secrets/{secret_name}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-selected-repositories-for-an-organization-secret
     */
    "PUT /orgs/{org}/actions/secrets/{secret_name}/repositories": Operation<"/orgs/{org}/actions/secrets/{secret_name}/repositories", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#add-selected-repository-to-an-organization-secret
     */
    "PUT /orgs/{org}/actions/secrets/{secret_name}/repositories/{repository_id}": Operation<"/orgs/{org}/actions/secrets/{secret_name}/repositories/{repository_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#block-a-user-from-an-organization
     */
    "PUT /orgs/{org}/blocks/{username}": Operation<"/orgs/{org}/blocks/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#set-interaction-restrictions-for-an-organization
     */
    "PUT /orgs/{org}/interaction-limits": Operation<"/orgs/{org}/interaction-limits", "put">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#set-organization-membership-for-a-user
     */
    "PUT /orgs/{org}/memberships/{username}": Operation<"/orgs/{org}/memberships/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#convert-an-organization-member-to-outside-collaborator
     */
    "PUT /orgs/{org}/outside_collaborators/{username}": Operation<"/orgs/{org}/outside_collaborators/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/orgs#set-public-organization-membership-for-the-authenticated-user
     */
    "PUT /orgs/{org}/public_members/{username}": Operation<"/orgs/{org}/public_members/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams#add-or-update-team-membership-for-a-user
     */
    "PUT /orgs/{org}/teams/{team_slug}/memberships/{username}": Operation<"/orgs/{org}/teams/{team_slug}/memberships/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams#add-or-update-team-project-permissions
     */
    "PUT /orgs/{org}/teams/{team_slug}/projects/{project_id}": Operation<"/orgs/{org}/teams/{team_slug}/projects/{project_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#add-or-update-team-repository-permissions
     */
    "PUT /orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}": Operation<"/orgs/{org}/teams/{team_slug}/repos/{owner}/{repo}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/projects#add-project-collaborator
     */
    "PUT /projects/{project_id}/collaborators/{username}": Operation<"/projects/{project_id}/collaborators/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-github-actions-permissions-for-a-repository
     */
    "PUT /repos/{owner}/{repo}/actions/permissions": Operation<"/repos/{owner}/{repo}/actions/permissions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#set-allowed-actions-for-a-repository
     */
    "PUT /repos/{owner}/{repo}/actions/permissions/selected-actions": Operation<"/repos/{owner}/{repo}/actions/permissions/selected-actions", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-or-update-a-repository-secret
     */
    "PUT /repos/{owner}/{repo}/actions/secrets/{secret_name}": Operation<"/repos/{owner}/{repo}/actions/secrets/{secret_name}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#disable-a-workflow
     */
    "PUT /repos/{owner}/{repo}/actions/workflows/{workflow_id}/disable": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}/disable", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#enable-a-workflow
     */
    "PUT /repos/{owner}/{repo}/actions/workflows/{workflow_id}/enable": Operation<"/repos/{owner}/{repo}/actions/workflows/{workflow_id}/enable", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#enable-automated-security-fixes
     */
    "PUT /repos/{owner}/{repo}/automated-security-fixes": Operation<"/repos/{owner}/{repo}/automated-security-fixes", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-branch-protection
     */
    "PUT /repos/{owner}/{repo}/branches/{branch}/protection": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#set-status-check-contexts
     */
    "PUT /repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/required_status_checks/contexts", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#set-app-access-restrictions
     */
    "PUT /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/apps", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#set-team-access-restrictions
     */
    "PUT /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/teams", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#set-user-access-restrictions
     */
    "PUT /repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users": Operation<"/repos/{owner}/{repo}/branches/{branch}/protection/restrictions/users", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#add-a-repository-collaborator
     */
    "PUT /repos/{owner}/{repo}/collaborators/{username}": Operation<"/repos/{owner}/{repo}/collaborators/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-or-update-file-contents
     */
    "PUT /repos/{owner}/{repo}/contents/{path}": Operation<"/repos/{owner}/{repo}/contents/{path}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#create-or-update-an-environment
     */
    "PUT /repos/{owner}/{repo}/environments/{environment_name}": Operation<"/repos/{owner}/{repo}/environments/{environment_name}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/migrations#start-an-import
     */
    "PUT /repos/{owner}/{repo}/import": Operation<"/repos/{owner}/{repo}/import", "put">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#set-interaction-restrictions-for-a-repository
     */
    "PUT /repos/{owner}/{repo}/interaction-limits": Operation<"/repos/{owner}/{repo}/interaction-limits", "put">;
    /**
     * @see https://docs.github.com/rest/reference/issues#set-labels-for-an-issue
     */
    "PUT /repos/{owner}/{repo}/issues/{issue_number}/labels": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/labels", "put">;
    /**
     * @see https://docs.github.com/rest/reference/issues#lock-an-issue
     */
    "PUT /repos/{owner}/{repo}/issues/{issue_number}/lock": Operation<"/repos/{owner}/{repo}/issues/{issue_number}/lock", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#enable-git-lfs-for-a-repository
     */
    "PUT /repos/{owner}/{repo}/lfs": Operation<"/repos/{owner}/{repo}/lfs", "put">;
    /**
     * @see https://docs.github.com/rest/reference/activity#mark-repository-notifications-as-read
     */
    "PUT /repos/{owner}/{repo}/notifications": Operation<"/repos/{owner}/{repo}/notifications", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#update-information-about-a-github-pages-site
     */
    "PUT /repos/{owner}/{repo}/pages": Operation<"/repos/{owner}/{repo}/pages", "put">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#merge-a-pull-request
     */
    "PUT /repos/{owner}/{repo}/pulls/{pull_number}/merge": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/merge", "put">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#update-a-review-for-a-pull-request
     */
    "PUT /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#dismiss-a-review-for-a-pull-request
     */
    "PUT /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/dismissals": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/dismissals", "put">;
    /**
     * @see https://docs.github.com/rest/reference/pulls#update-a-pull-request-branch
     */
    "PUT /repos/{owner}/{repo}/pulls/{pull_number}/update-branch": Operation<"/repos/{owner}/{repo}/pulls/{pull_number}/update-branch", "put">;
    /**
     * @see https://docs.github.com/rest/reference/activity#set-a-repository-subscription
     */
    "PUT /repos/{owner}/{repo}/subscription": Operation<"/repos/{owner}/{repo}/subscription", "put">;
    /**
     * @see https://docs.github.com/rest/reference/repos#replace-all-repository-topics
     */
    "PUT /repos/{owner}/{repo}/topics": Operation<"/repos/{owner}/{repo}/topics", "put", "mercy">;
    /**
     * @see https://docs.github.com/rest/reference/repos#enable-vulnerability-alerts
     */
    "PUT /repos/{owner}/{repo}/vulnerability-alerts": Operation<"/repos/{owner}/{repo}/vulnerability-alerts", "put">;
    /**
     * @see https://docs.github.com/rest/reference/actions#create-or-update-an-environment-secret
     */
    "PUT /repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}": Operation<"/repositories/{repository_id}/environments/{environment_name}/secrets/{secret_name}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-scim-information-for-a-provisioned-enterprise-group
     */
    "PUT /scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}": Operation<"/scim/v2/enterprises/{enterprise}/Groups/{scim_group_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/enterprise-admin#set-scim-information-for-a-provisioned-enterprise-user
     */
    "PUT /scim/v2/enterprises/{enterprise}/Users/{scim_user_id}": Operation<"/scim/v2/enterprises/{enterprise}/Users/{scim_user_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/scim#set-scim-information-for-a-provisioned-user
     */
    "PUT /scim/v2/organizations/{org}/Users/{scim_user_id}": Operation<"/scim/v2/organizations/{org}/Users/{scim_user_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams#add-team-member-legacy
     */
    "PUT /teams/{team_id}/members/{username}": Operation<"/teams/{team_id}/members/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams#add-or-update-team-membership-for-a-user-legacy
     */
    "PUT /teams/{team_id}/memberships/{username}": Operation<"/teams/{team_id}/memberships/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#add-or-update-team-project-permissions-legacy
     */
    "PUT /teams/{team_id}/projects/{project_id}": Operation<"/teams/{team_id}/projects/{project_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/teams/#add-or-update-team-repository-permissions-legacy
     */
    "PUT /teams/{team_id}/repos/{owner}/{repo}": Operation<"/teams/{team_id}/repos/{owner}/{repo}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/users#block-a-user
     */
    "PUT /user/blocks/{username}": Operation<"/user/blocks/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/users#follow-a-user
     */
    "PUT /user/following/{username}": Operation<"/user/following/{username}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/apps#add-a-repository-to-an-app-installation
     */
    "PUT /user/installations/{installation_id}/repositories/{repository_id}": Operation<"/user/installations/{installation_id}/repositories/{repository_id}", "put">;
    /**
     * @see https://docs.github.com/rest/reference/interactions#set-interaction-restrictions-for-your-public-repositories
     */
    "PUT /user/interaction-limits": Operation<"/user/interaction-limits", "put">;
    /**
     * @see https://docs.github.com/rest/reference/activity#star-a-repository-for-the-authenticated-user
     */
    "PUT /user/starred/{owner}/{repo}": Operation<"/user/starred/{owner}/{repo}", "put">;
}
export {};
