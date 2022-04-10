import {
  OctokitResponse,
  RequestOptions,
  ResponseHeaders,
} from "/-/@octokit/types@v6.33.0-CLItWMbWROrzbdY30nuK/dist=es2019,mode=types/index.d.ts";
export declare type RequestErrorOptions = {
  /** @deprecated set `response` instead */
  headers?: ResponseHeaders;
  request: RequestOptions;
} | {
  response: OctokitResponse<unknown>;
  request: RequestOptions;
};
