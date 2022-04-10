import { RequestHeaders } from "./RequestHeaders.d.ts";
import { RequestMethod } from "./RequestMethod.d.ts";
import { RequestParameters } from "./RequestParameters.d.ts";
import { Url } from "./Url.d.ts";
/**
 * The `.endpoint()` method is guaranteed to set all keys defined by RequestParameters
 * as well as the method property.
 */
export declare type EndpointDefaults = RequestParameters & {
  baseUrl: Url;
  method: RequestMethod;
  url?: Url;
  headers: RequestHeaders & {
    accept: string;
    "user-agent": string;
  };
  mediaType: {
    format: string;
    previews: string[];
  };
};
