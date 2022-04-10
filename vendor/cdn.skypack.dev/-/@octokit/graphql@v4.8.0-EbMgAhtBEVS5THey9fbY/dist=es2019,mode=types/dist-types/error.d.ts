import { ResponseHeaders } from "/-/@octokit/types@v6.34.0-YyJmQq3Ajv0ACapsoOVh/dist=es2019,mode=types/index.d.ts";
import { GraphQlEndpointOptions, GraphQlQueryResponse } from "./types.d.ts";
declare type ServerResponseData<T> = Required<GraphQlQueryResponse<T>>;
export declare class GraphqlResponseError<ResponseData> extends Error {
    readonly request: GraphQlEndpointOptions;
    readonly headers: ResponseHeaders;
    readonly response: ServerResponseData<ResponseData>;
    name: string;
    readonly errors: GraphQlQueryResponse<never>["errors"];
    readonly data: ResponseData;
    constructor(request: GraphQlEndpointOptions, headers: ResponseHeaders, response: ServerResponseData<ResponseData>);
}
export {};
