import { endpoint as endpoint2 } from "/-/@octokit/endpoint@v6.0.12-uRrGy3NGbXOw4pbkNPpd/dist=es2019,mode=imports/optimized/@octokit/endpoint.js";
import { getUserAgent } from "/-/universal-user-agent@v6.0.0-fUAPE3UH5QP7qG0fd0dH/dist=es2019,mode=imports/optimized/universal-user-agent.js";
import { isPlainObject } from "/-/is-plain-object@v5.0.0-8mrVMp9y5RYdpZYGe1Tt/dist=es2019,mode=imports/optimized/is-plain-object.js";
import { RequestError } from "/-/@octokit/request-error@v2.1.0-eEdMdUdHjpMHPFvA4aCp/dist=es2019,mode=imports/optimized/@octokit/request-error.js";
var getGlobal = function () {
  if (typeof self !== "undefined") {
    return self;
  }
  if (typeof window !== "undefined") {
    return window;
  }
  if (typeof global !== "undefined") {
    return global;
  }
  throw new Error("unable to locate global object");
};
var global = getGlobal();
var nodeFetch = global.fetch.bind(global);
const Headers = global.Headers;
const Request = global.Request;
const Response = global.Response;
const VERSION = "5.6.3";
function getBufferResponse(response) {
  return response.arrayBuffer();
}
function fetchWrapper(requestOptions) {
  const log = requestOptions.request && requestOptions.request.log
    ? requestOptions.request.log
    : console;
  if (
    isPlainObject(requestOptions.body) || Array.isArray(requestOptions.body)
  ) {
    requestOptions.body = JSON.stringify(requestOptions.body);
  }
  let headers = {};
  let status;
  let url;
  const fetch = requestOptions.request && requestOptions.request.fetch ||
    nodeFetch;
  return fetch(
    requestOptions.url,
    Object.assign({
      method: requestOptions.method,
      body: requestOptions.body,
      headers: requestOptions.headers,
      redirect: requestOptions.redirect,
    }, requestOptions.request),
  ).then(async (response) => {
    url = response.url;
    status = response.status;
    for (const keyAndValue of response.headers) {
      headers[keyAndValue[0]] = keyAndValue[1];
    }
    if ("deprecation" in headers) {
      const matches = headers.link &&
        headers.link.match(/<([^>]+)>; rel="deprecation"/);
      const deprecationLink = matches && matches.pop();
      log.warn(
        `[@octokit/request] "${requestOptions.method} ${requestOptions.url}" is deprecated. It is scheduled to be removed on ${headers.sunset}${
          deprecationLink ? `. See ${deprecationLink}` : ""
        }`,
      );
    }
    if (status === 204 || status === 205) {
      return;
    }
    if (requestOptions.method === "HEAD") {
      if (status < 400) {
        return;
      }
      throw new RequestError(response.statusText, status, {
        response: {
          url,
          status,
          headers,
          data: void 0,
        },
        request: requestOptions,
      });
    }
    if (status === 304) {
      throw new RequestError("Not modified", status, {
        response: {
          url,
          status,
          headers,
          data: await getResponseData(response),
        },
        request: requestOptions,
      });
    }
    if (status >= 400) {
      const data = await getResponseData(response);
      const error = new RequestError(toErrorMessage(data), status, {
        response: {
          url,
          status,
          headers,
          data,
        },
        request: requestOptions,
      });
      throw error;
    }
    return getResponseData(response);
  }).then((data) => {
    return {
      status,
      url,
      headers,
      data,
    };
  }).catch((error) => {
    if (error instanceof RequestError) {
      throw error;
    }
    throw new RequestError(error.message, 500, {
      request: requestOptions,
    });
  });
}
async function getResponseData(response) {
  const contentType = response.headers.get("content-type");
  if (/application\/json/.test(contentType)) {
    return response.json();
  }
  if (!contentType || /^text\/|charset=utf-8$/.test(contentType)) {
    return response.text();
  }
  return getBufferResponse(response);
}
function toErrorMessage(data) {
  if (typeof data === "string") {
    return data;
  }
  if ("message" in data) {
    if (Array.isArray(data.errors)) {
      return `${data.message}: ${data.errors.map(JSON.stringify).join(", ")}`;
    }
    return data.message;
  }
  return `Unknown error: ${JSON.stringify(data)}`;
}
function withDefaults(oldEndpoint, newDefaults) {
  const endpoint3 = oldEndpoint.defaults(newDefaults);
  const newApi = function (route, parameters) {
    const endpointOptions = endpoint3.merge(route, parameters);
    if (!endpointOptions.request || !endpointOptions.request.hook) {
      return fetchWrapper(endpoint3.parse(endpointOptions));
    }
    const request2 = (route2, parameters2) => {
      return fetchWrapper(
        endpoint3.parse(endpoint3.merge(route2, parameters2)),
      );
    };
    Object.assign(request2, {
      endpoint: endpoint3,
      defaults: withDefaults.bind(null, endpoint3),
    });
    return endpointOptions.request.hook(request2, endpointOptions);
  };
  return Object.assign(newApi, {
    endpoint: endpoint3,
    defaults: withDefaults.bind(null, endpoint3),
  });
}
const request = withDefaults(endpoint2, {
  headers: {
    "user-agent": `octokit-request.js/${VERSION} ${getUserAgent()}`,
  },
});
export { request };
export default null;
