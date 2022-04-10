import { EndpointInterface } from "./EndpointInterface.d.ts";
import { OctokitResponse } from "./OctokitResponse.d.ts";
import { RequestParameters } from "./RequestParameters.d.ts";
import { Route } from "./Route.d.ts";
import { Endpoints } from "./generated/Endpoints.d.ts";
export interface RequestInterface<D extends object = object> {
  /**
   * Sends a request based on endpoint options
   *
   * @param {object} endpoint Must set `method` and `url`. Plus URL, query or body parameters, as well as `headers`, `mediaType.{format|previews}`, `request`, or `baseUrl`.
   */
  <T = any, O extends RequestParameters = RequestParameters>(
    options:
      & O
      & {
        method?: string;
      }
      & ("url" extends keyof D ? {
        url?: string;
      } : {
        url: string;
      }),
  ): Promise<OctokitResponse<T>>;
  /**
   * Sends a request based on endpoint options
   *
   * @param {string} route Request method + URL. Example: `'GET /orgs/{org}'`
   * @param {object} [parameters] URL, query or body parameters, as well as `headers`, `mediaType.{format|previews}`, `request`, or `baseUrl`.
   */
  <R extends Route>(
    route: keyof Endpoints | R,
    options?: R extends keyof Endpoints
      ? Endpoints[R]["parameters"] & RequestParameters
      : RequestParameters,
  ): R extends keyof Endpoints ? Promise<Endpoints[R]["response"]>
    : Promise<OctokitResponse<any>>;
  /**
   * Returns a new `request` with updated route and parameters
   */
  defaults: <O extends RequestParameters = RequestParameters>(
    newDefaults: O,
  ) => RequestInterface<D & O>;
  /**
   * Octokit endpoint API, see {@link https://github.com/octokit/endpoint.js|@octokit/endpoint}
   */
  endpoint: EndpointInterface<D>;
}
