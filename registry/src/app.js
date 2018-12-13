// TODO Cron job to update the database with current values.
const DATABASE = require("./database.json");
const { assert } = console;

const notFound = {
  status: "404",
  headers: {
    "Content-Type": [
      {
        key: "Content-Type",
        value: "text/plain"
      }
    ]
  },
  body: "Not Found\r\n"
};

function proxy(pathname) {
  if (!pathname.startsWith("/x/")) {
    return null;
  }

  const i = pathname.indexOf("/", 3);
  const rest = pathname.slice(i + 1);
  const nameBranch = pathname.slice(3, i);
  let [name, branch] = nameBranch.split("@", 2);
  const urlPattern = DATABASE[name];

  if (!branch) {
    branch = "master";
  }

  if (!urlPattern) {
    return null;
  }

  const url = urlPattern.replace("${b}", branch);
  assert(url.endsWith("/"));
  assert(!rest.startsWith("/"));
  return url + rest;
}
exports.proxy = proxy;

const docs = `
  <h1>deno.land packages</h1>
  <p>
    This is a URL redirection service for Deno scripts.
    The basic format is
    <code>https://deno.land/x/NAME@BRANCH/SCRIPT.ts</code>
    If you leave out the branch, it will default to master.
  </p>
  <p>
    To add a package send a pull request to
    <code>https://github.com/denoland/registry/blob/master/x/database.json</code>
  </p>
`;

function indexPage() {
  let body = `<!DOCTYPE html><html>${docs}<ul>`;

  for (let name in DATABASE) {
    const url = DATABASE[name];
    body += `<li>https://deno.land/x/${name}/  &rarr;  ${url} </li>\n`;
  }

  body += "</ul></html>";
  return {
    status: "200",
    body,
    headers: {
      "Content-Type": [
        {
          key: "Content-Type",
          value: "text/html"
        }
      ]
    }
  };
}
exports.indexPage = indexPage;

exports.lambdaHandler = (event, context, callback) => {
  console.log("Received event:", JSON.stringify(event, null, 2));
  const pathname = event.Records[0].cf.request.uri;
  console.log("pathname", pathname);

  if (pathname === "/x/" || pathname === "/x" || pathname === "/x/index.html") {
    callback(null, indexPage());
    return;
  }

  const l = proxy(pathname);
  if (!l) {
    callback(null, notFound);
    return;
  }

  console.log("redirect", pathname, l);
  const response = {
    status: "302",
    headers: {
      location: [
        {
          key: "Location",
          value: l
        }
      ]
    }
  };
  callback(null, response);
};
