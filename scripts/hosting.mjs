import { githubHosting } from "./hosting/github.mjs";

export function createHosting(config, options) {
  if (config.hosting.provider === "github") return githubHosting(options);
  throw new Error(
    `Unsupported hosting provider: ${config.hosting.provider}; implement an explicit adapter and its workflows first`,
  );
}
