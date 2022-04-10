import {
  EndpointInterface,
  EndpointOptions,
  RequestParameters as RequestParametersType,
} from "/-/@octokit/types@v6.34.0-YyJmQq3Ajv0ACapsoOVh/dist=es2019,mode=types/index.d.ts";
export declare type GraphQlEndpointOptions = EndpointOptions & {
  variables?: {
    [key: string]: unknown;
  };
};
export declare type RequestParameters = RequestParametersType;
export declare type Query = string;
export interface graphql {
  /**
   * Sends a GraphQL query request based on endpoint options
   * The GraphQL query must be specified in `options`.
   *
   * @param {object} endpoint Must set `method` and `url`. Plus URL, query or body parameters, as well as `headers`, `mediaType.{format|previews}`, `request`, or `baseUrl`.
   */
  <ResponseData>(options: RequestParameters): GraphQlResponse<ResponseData>;
  /**
   * Sends a GraphQL query request based on endpoint options
   *
   * @param {string} query GraphQL query. Example: `'query { viewer { login } }'`.
   * @param {object} [parameters] URL, query or body parameters, as well as `headers`, `mediaType.{format|previews}`, `request`, or `baseUrl`.
   */
  <ResponseData>(
    query: Query,
    parameters?: RequestParameters,
  ): GraphQlResponse<ResponseData>;
  /**
   * Returns a new `endpoint` with updated route and parameters
   */
  defaults: (newDefaults: RequestParameters) => graphql;
  /**
   * Octokit endpoint API, see {@link https://github.com/octokit/endpoint.js|@octokit/endpoint}
   */
  endpoint: EndpointInterface;
}
export declare type GraphQlResponse<ResponseData> = Promise<ResponseData>;
export declare type GraphQlQueryResponseData = {
  [key: string]: any;
};
export declare type GraphQlQueryResponse<ResponseData> = {
  data: ResponseData;
  errors?: [
    {
      type: string;
      message: string;
      path: [string];
      extensions: {
        [key: string]: any;
      };
      locations: [
        {
          line: number;
          column: number;
        },
      ];
    },
  ];
};
