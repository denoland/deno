import { RequestMethod } from "./RequestMethod.d.ts";
import { Url } from "./Url.d.ts";
import { RequestParameters } from "./RequestParameters.d.ts";
export declare type EndpointOptions = RequestParameters & {
  method: RequestMethod;
  url: Url;
};
