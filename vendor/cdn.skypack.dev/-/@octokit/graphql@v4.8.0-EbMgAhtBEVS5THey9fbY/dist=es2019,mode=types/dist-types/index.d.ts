import { request } from "/-/@octokit/request@v5.6.3-8MiyZSoy8B73C1K9nYC8/dist=es2019,mode=types/index.d.ts";
export declare const graphql: import("./types.d.ts").graphql;
export { GraphQlQueryResponseData } from "./types.d.ts";
export { GraphqlResponseError } from "./error.d.ts";
export declare function withCustomRequest(customRequest: typeof request): import("./types.d.ts").graphql;
