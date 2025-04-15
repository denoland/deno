// Copyright 2018-2025 the Deno authors. MIT license.

export const STATUS_CODES = {
  /** RFC 7231, 6.2.1 */
  100: "Continue",
  /** RFC 7231, 6.2.2 */
  101: "Switching Protocols",
  /** RFC 2518, 10.1 */
  102: "Processing",
  /** RFC 8297 **/
  103: "Early Hints",

  /** RFC 7231, 6.3.1 */
  200: "OK",
  /** RFC 7231, 6.3.2 */
  201: "Created",
  /** RFC 7231, 6.3.3 */
  202: "Accepted",
  /** RFC 7231, 6.3.4 */
  203: "Non-Authoritative Information",
  /** RFC 7231, 6.3.5 */
  204: "No Content",
  /** RFC 7231, 6.3.6 */
  205: "Reset Content",
  /** RFC 7233, 4.1 */
  206: "Partial Content",
  /** RFC 4918, 11.1 */
  207: "Multi-Status",
  /** RFC 5842, 7.1 */
  208: "Already Reported",
  /** RFC 3229, 10.4.1 */
  226: "IM Used",

  /** RFC 7231, 6.4.1 */
  300: "Multiple Choices",
  /** RFC 7231, 6.4.2 */
  301: "Moved Permanently",
  /** RFC 7231, 6.4.3 */
  302: "Found",
  /** RFC 7231, 6.4.4 */
  303: "See Other",
  /** RFC 7232, 4.1 */
  304: "Not Modified",
  /** RFC 7231, 6.4.5 */
  305: "Use Proxy",
  /** RFC 7231, 6.4.7 */
  307: "Temporary Redirect",
  /** RFC 7538, 3 */
  308: "Permanent Redirect",

  /** RFC 7231, 6.5.1 */
  400: "Bad Request",
  /** RFC 7235, 3.1 */
  401: "Unauthorized",
  /** RFC 7231, 6.5.2 */
  402: "Payment Required",
  /** RFC 7231, 6.5.3 */
  403: "Forbidden",
  /** RFC 7231, 6.5.4 */
  404: "Not Found",
  /** RFC 7231, 6.5.5 */
  405: "Method Not Allowed",
  /** RFC 7231, 6.5.6 */
  406: "Not Acceptable",
  /** RFC 7235, 3.2 */
  407: "Proxy Authentication Required",
  /** RFC 7231, 6.5.7 */
  408: "Request Timeout",
  /** RFC 7231, 6.5.8 */
  409: "Conflict",
  /** RFC 7231, 6.5.9 */
  410: "Gone",
  /** RFC 7231, 6.5.10 */
  411: "Length Required",
  /** RFC 7232, 4.2 */
  412: "Precondition Failed",
  /** RFC 7231, 6.5.11 */
  413: "Payload Too Large",
  /** RFC 7231, 6.5.12 */
  414: "URI Too Long",
  /** RFC 7231, 6.5.13 */
  415: "Unsupported Media Type",
  /** RFC 7233, 4.4 */
  416: "Range Not Satisfiable",
  /** RFC 7231, 6.5.14 */
  417: "Expectation Failed",
  /** RFC 7168, 2.3.3 */
  418: "I'm a Teapot",
  /** RFC 7540, 9.1.2 */
  421: "Misdirected Request",
  /** RFC 4918, 11.2 */
  422: "Unprocessable Entity",
  /** RFC 4918, 11.3 */
  423: "Locked",
  /** RFC 4918, 11.4 */
  424: "Failed Dependency",
  /** RFC 8470, 5.2 */
  425: "Too Early",
  /** RFC 7231, 6.5.15 */
  426: "Upgrade Required",
  /** RFC 6585, 3 */
  428: "Precondition Required",
  /** RFC 6585, 4 */
  429: "Too Many Requests",
  /** RFC 6585, 5 */
  431: "Request Header Fields Too Large",
  /** RFC 7725, 3 */
  451: "Unavailable For Legal Reasons",

  /** RFC 7231, 6.6.1 */
  500: "Internal Server Error",
  /** RFC 7231, 6.6.2 */
  501: "Not Implemented",
  /** RFC 7231, 6.6.3 */
  502: "Bad Gateway",
  /** RFC 7231, 6.6.4 */
  503: "Service Unavailable",
  /** RFC 7231, 6.6.5 */
  504: "Gateway Timeout",
  /** RFC 7231, 6.6.6 */
  505: "HTTP Version Not Supported",
  /** RFC 2295, 8.1 */
  506: "Variant Also Negotiates",
  /** RFC 4918, 11.5 */
  507: "Insufficient Storage",
  /** RFC 5842, 7.2 */
  508: "Loop Detected",
  /** RFC 2774, 7 */
  510: "Not Extended",
  /** RFC 6585, 6 */
  511: "Network Authentication Required",
};

export default {
  STATUS_CODES,
};
