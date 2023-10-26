import { createPrivateKey } from "node:crypto";
import { App, Octokit } from "npm:octokit@^2.0.10";

const app = new App({
  appId: " ",
  privateKey: " "
});

const { data } = await app.octokit.request("/app");

app.log.warn("ok")
// createPrivateKey();A