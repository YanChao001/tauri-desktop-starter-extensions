import { readFileSync } from "node:fs";
import { readdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { assertUnlinkedPath, parseJson, readBounded } from "./files.mjs";
import { validRepository } from "./protocol.mjs";
import { applyRepositoryChanges } from "./repository-files.mjs";

const root = resolve(import.meta.dirname, "..");
const linkTokens = {
  file: ["repositoryUrl", "ref", "path"],
  tree: ["repositoryUrl", "ref", "path"],
  commit: ["repositoryUrl", "ref"],
  commits: ["repositoryUrl", "ref"],
  compare: ["repositoryUrl", "base", "ref"],
  release: ["repositoryUrl", "tag"],
};
const object = (value, keys) =>
  value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).every((key) => keys.includes(key)) &&
  keys.every((key) => Object.hasOwn(value, key));
const validRef = (value) =>
  typeof value === "string" &&
  value.length <= 200 &&
  /^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(value) &&
  !value.split("/").some((part) => !part || part === "." || part === "..");

function httpsUrl(value, base = false) {
  if (
    typeof value !== "string" ||
    value.length > 2048 ||
    !value.isWellFormed() ||
    // eslint-disable-next-line no-control-regex -- Reject control bytes in configured URLs.
    /[\u0000-\u0020\u007f<>"'()\\{}]/.test(value)
  )
    throw new Error("Invalid HTTPS repository URL");
  const url = new URL(value);
  if (
    url.protocol !== "https:" ||
    !url.hostname ||
    url.username ||
    url.password ||
    (base && (url.search || url.hash || value.endsWith("/")))
  )
    throw new Error("Invalid HTTPS repository URL");
  return value;
}

function template(value, tokens) {
  if (typeof value !== "string" || value.length > 2048)
    throw new Error("Invalid repository link template");
  const found = [...value.matchAll(/\{([^{}]+)\}/g)].map((match) => match[1]);
  if (
    found.some((token) => !tokens.includes(token)) ||
    tokens.some((token) => !found.includes(token)) ||
    /[{}]/.test(value.replace(/\{[^{}]+\}/g, ""))
  )
    throw new Error("Unknown or missing repository template placeholder");
  return value;
}

const substitute = (value, replacements) =>
  value.replace(/\{([^{}]+)\}/g, (_match, token) => replacements[token]);

export function validateRepositoryConfig(config) {
  if (
    !object(config, [
      "schemaVersion",
      "hosting",
      "host",
      "wallpaperUserAgentTemplate",
    ]) ||
    config.schemaVersion !== 1 ||
    !object(config.hosting, ["provider", "repositoryUrlTemplate", "links"]) ||
    config.hosting.provider !== "github" ||
    !object(config.hosting.links, Object.keys(linkTokens)) ||
    !object(config.host, [
      "repository",
      "repositoryUrl",
      "docsRef",
      "defaultRef",
    ]) ||
    !validRepository(config.host.repository) ||
    !validRef(config.host.docsRef) ||
    !validRef(config.host.defaultRef)
  )
    throw new Error("Invalid repository configuration");
  template(config.hosting.repositoryUrlTemplate, ["repository"]);
  const sample = repositoryUrl(config, "owner/market");
  // The repository placeholder is a path, never an authority or a query value.
  if (
    !config.hosting.repositoryUrlTemplate.startsWith(
      `${new URL(sample).origin}/`,
    )
  )
    throw new Error("Invalid repository URL template authority");
  httpsUrl(config.host.repositoryUrl, true);
  for (const [kind, tokens] of Object.entries(linkTokens)) {
    const value = template(config.hosting.links[kind], tokens);
    if (!value.startsWith("{repositoryUrl}/"))
      throw new Error("Invalid repository link template prefix");
    httpsUrl(
      substitute(value, {
        repositoryUrl: sample,
        ref: "main",
        path: "docs/example.md",
        base: "v1",
        tag: "v2",
      }),
    );
  }
  template(config.wallpaperUserAgentTemplate, ["repositoryUrl"]);
  if (
    !/^[\x20-\x7e]+$/.test(config.wallpaperUserAgentTemplate) ||
    config.wallpaperUserAgentTemplate.length > 512
  )
    throw new Error("Invalid wallpaper User-Agent template");
  return config;
}

/** Read-only entry point; importing this module does not inspect or change a checkout. */
export function readRepositoryConfig(directory = root) {
  const bytes = readFileSync(join(directory, "repository.config.json"));
  if (bytes.length > 64 * 1024)
    throw new Error("Invalid repository configuration size");
  return validateRepositoryConfig(parseJson(bytes));
}

export function repositoryUrl(config, repository) {
  if (!validRepository(repository))
    throw new Error("Invalid repository identity");
  return httpsUrl(
    substitute(template(config.hosting.repositoryUrlTemplate, ["repository"]), {
      repository,
    }),
    true,
  );
}

function renderLink(config, baseUrl, kind, values) {
  const tokens = linkTokens[kind];
  if (!tokens) throw new Error("Unknown repository link kind");
  const replacements = { repositoryUrl: baseUrl };
  for (const token of tokens.filter((token) => token !== "repositoryUrl")) {
    const value = values[token];
    if (
      typeof value !== "string" ||
      !value ||
      // eslint-disable-next-line no-control-regex -- Link values must not contain control bytes.
      /[\u0000-\u001f\u007f]/.test(value) ||
      (token === "path" &&
        value.split("/").some((part) => !part || part === "." || part === ".."))
    )
      throw new Error("Invalid repository link value");
    replacements[token] =
      token === "path"
        ? value.split("/").map(encodeURIComponent).join("/")
        : encodeURIComponent(value);
  }
  const link = httpsUrl(
    substitute(template(config.hosting.links[kind], tokens), replacements),
  );
  return values.fragment
    ? `${link}#${encodeURIComponent(values.fragment)}`
    : link;
}

export function repositoryLink(config, repository, kind, values = {}) {
  validateRepositoryConfig(config);
  return renderLink(config, repositoryUrl(config, repository), kind, values);
}

export function wallpaperUserAgent(config, repository) {
  validateRepositoryConfig(config);
  return substitute(config.wallpaperUserAgentTemplate, {
    repositoryUrl: repositoryUrl(config, repository),
  });
}

const escapeRegex = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function rewriteUrl(url, previous, current, oldBase, newBase, host) {
  if (
    url !== oldBase &&
    !url.startsWith(`${oldBase}/`) &&
    !url.startsWith(`${oldBase}#`) &&
    !url.startsWith(`${oldBase}?`)
  )
    return url;
  for (const kind of Object.keys(linkTokens)) {
    const captures = [];
    const pattern = previous.hosting.links[kind]
      .split(/(\{[^{}]+\})/g)
      .map((part) => {
        if (part === "{repositoryUrl}") return escapeRegex(oldBase);
        if (part.startsWith("{")) {
          const token = part.slice(1, -1);
          captures.push(token);
          return token === "path" ? "([^?#]+)" : "([^/?#]+?)";
        }
        return escapeRegex(part);
      })
      .join("");
    const match = new RegExp(`^${pattern}(#[^?]*)?$`).exec(url);
    if (!match) continue;
    const values = Object.fromEntries(
      captures.map((token, index) => [
        token,
        decodeURIComponent(match[index + 1]),
      ]),
    );
    if (
      oldBase === newBase &&
      previous.hosting.links[kind] === current.hosting.links[kind] &&
      (!host ||
        values.ref !== previous.host.docsRef ||
        previous.host.docsRef === current.host.docsRef)
    )
      return url;
    if (host && values.ref === previous.host.docsRef)
      values.ref = current.host.docsRef;
    if (match[captures.length + 1])
      values.fragment = decodeURIComponent(match[captures.length + 1].slice(1));
    return renderLink(current, newBase, kind, values);
  }
  return `${newBase}${url.slice(oldBase.length)}`;
}

function rewriteText(source, previous, current, oldRepository, repository) {
  const oldMarket = repositoryUrl(previous, oldRepository),
    newMarket = repositoryUrl(current, repository);
  let result = source.replace(/https:\/\/[^\s<>"'`()]+/g, (url) => {
    const host = rewriteUrl(
      url,
      previous,
      current,
      previous.host.repositoryUrl,
      current.host.repositoryUrl,
      true,
    );
    return host !== url
      ? host
      : rewriteUrl(url, previous, current, oldMarket, newMarket, false);
  });
  // CLI examples contain the project identity without its URL. Match complete slugs only.
  for (const [before, after] of [
    [oldRepository, repository],
    [previous.host.repository, current.host.repository],
  ]) {
    result = result.replace(
      new RegExp(
        `(?<![A-Za-z0-9._/-])${escapeRegex(before)}(?![A-Za-z0-9._/-])`,
        "g",
      ),
      after,
    );
  }
  return result;
}

async function proseFiles(directory, prefix = "") {
  const files = [];
  for (const entry of await readdir(join(directory, prefix), {
    withFileTypes: true,
  })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isSymbolicLink()) continue;
    if (
      entry.isDirectory() &&
      (relative === "docs" ||
        relative.startsWith("docs/") ||
        relative === "plugins" ||
        /^plugins\/[^/]+$/.test(relative))
    )
      files.push(...(await proseFiles(directory, relative)));
    else if (entry.isFile() && entry.name.endsWith(".md")) files.push(relative);
  }
  return files;
}

/** Sync only project prose and explicitly registered generated integration fields. */
export async function synchronizeRepository({
  directory = root,
  check = false,
  beforeCommit,
} = {}) {
  directory = resolve(directory);
  await assertUnlinkedPath(directory, true);
  const config = validateRepositoryConfig(
    parseJson(
      await readBounded(join(directory, "repository.config.json"), 64 * 1024),
    ),
  );
  const { repository } = parseJson(
    await readBounded(join(directory, "listings.json"), 64 * 1024),
  );
  if (!validRepository(repository))
    throw new Error("Invalid repository identity in listings.json");
  const state = parseJson(
    await readBounded(join(directory, "repository.generated.json"), 64 * 1024),
  );
  if (
    !object(state, ["schemaVersion", "repository", "config"]) ||
    state.schemaVersion !== 1 ||
    !validRepository(state.repository)
  )
    throw new Error("Invalid repository synchronization state");
  validateRepositoryConfig(state.config);
  const updates = [];
  async function update(name, transform, allowMissing = false) {
    let before;
    try {
      before = await readBounded(join(directory, name), 2 * 1024 * 1024);
    } catch (error) {
      if (!allowMissing || error.code !== "ENOENT") throw error;
      await assertUnlinkedPath(dirname(join(directory, name)), true);
    }
    const after = transform(before?.toString("utf8") ?? "");
    if (!before?.equals(Buffer.from(after, "utf8")))
      updates.push({ name, before, after });
  }
  for (const name of await proseFiles(directory))
    await update(name, (text) =>
      rewriteText(text, state.config, config, state.repository, repository),
    );
  await update(".github/actions/host-tool/action.yml", (text) => {
    if (!/^ {8}repository: [^\r\n]+$/m.test(text))
      throw new Error("Missing generated host-tool repository field");
    return text.replace(
      /^ {8}repository: [^\r\n]+$/m,
      `        repository: ${config.host.repository}`,
    );
  });
  await update(".github/workflows/ci.yml", (text) => {
    if (!/vars\.STARTER_HOST_REF \|\| '[^'\r\n]+'/.test(text))
      throw new Error("Missing generated CI host default ref");
    return text.replace(
      /vars\.STARTER_HOST_REF \|\| '[^'\r\n]+'/,
      `vars.STARTER_HOST_REF || '${config.host.defaultRef}'`,
    );
  });
  await update("plugins/wallpaper-browser/smoke.json", (text) => {
    if (!/"User-Agent"\s*:\s*"(?:[^"\\]|\\.)*"/.test(text))
      throw new Error("Missing wallpaper User-Agent fixture");
    return text.replace(
      /("User-Agent"\s*:\s*)"(?:[^"\\]|\\.)*"/g,
      (_match, prefix) =>
        `${prefix}${JSON.stringify(wallpaperUserAgent(config, repository))}`,
    );
  });
  await update(
    "plugins/wallpaper-browser/user-agent.generated.txt",
    () => `${wallpaperUserAgent(config, repository)}\n`,
    true,
  );
  const nextState = `${JSON.stringify({ schemaVersion: 1, repository, config }, null, 2)}\n`;
  await update("repository.generated.json", () => nextState);
  if (check && updates.length)
    throw new Error(
      `Repository configuration drift; run repository:sync:\n${updates.map(({ name }) => `- ${name}`).join("\n")}`,
    );
  if (!check)
    await applyRepositoryChanges(directory, updates, { beforeCommit });
  return updates.map(({ name }) => name);
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const [command, flag, selectedRoot] = process.argv.slice(2);
  if (
    !["sync", "check"].includes(command) ||
    (flag !== undefined &&
      (flag !== "--root" || !selectedRoot || process.argv.length !== 5))
  )
    throw new Error(
      "Usage: node scripts/repository-config.mjs <sync|check> [--root <checkout>]",
    );
  const changed = await synchronizeRepository({
    directory: selectedRoot ? resolve(selectedRoot) : root,
    check: command === "check",
  });
  console.log(
    command === "check"
      ? "Repository configuration is synchronized"
      : `Synchronized ${changed.length} repository configuration file(s)`,
  );
}
