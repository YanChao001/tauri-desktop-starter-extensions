import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { readRepositoryConfig } from "./repository-config.mjs";
import { CATALOG_LIFETIME } from "./protocol.mjs";
import { verifyAssets } from "./verify.mjs";

const git = (...args) =>
  execFileSync("git", args, { encoding: "utf8", windowsHide: true }).trim();
const request = (endpoint) =>
  JSON.parse(
    execFileSync("gh", ["api", endpoint], {
      encoding: "utf8",
      windowsHide: true,
      timeout: 30_000,
    }),
  );

export function requiresFullCheck(paths) {
  return (
    !paths.length ||
    paths.some(
      (path) =>
        !(
          path.endsWith(".md") ||
          path.endsWith(".test.mjs") ||
          path === "scripts/ci-plan.mjs" ||
          path === ".github/workflows/ci.yml" ||
          path === ".github/workflows/publish.yml"
        ),
    )
  );
}

export function checkedBuildName(tree, host) {
  if (![tree, host].every((value) => /^[a-f\d]{40}$/i.test(value)))
    throw new Error("Expected market tree and immutable host SHA");
  return `market-checked-v1-${tree.toLowerCase()}-${host.toLowerCase()}`;
}

export function findCheckedBuild(
  name,
  { repository, api = request, warn = console.warn } = {},
) {
  try {
    const prefix = `repos/${repository}/actions`;
    const { artifacts } = api(`${prefix}/artifacts?name=${name}&per_page=100`);
    for (const artifact of artifacts) {
      const source = artifact.workflow_run;
      if (
        artifact.name !== name ||
        artifact.expired ||
        !source?.repository_id ||
        source.head_repository_id !== source.repository_id
      )
        continue;
      const run = api(`${prefix}/runs/${source.id}`);
      if (
        run.path === ".github/workflows/ci.yml" &&
        run.status === "completed" &&
        run.conclusion === "success"
      )
        return run;
    }
  } catch (error) {
    warn(`Unable to reuse checked packages; running checks: ${error.message}`);
  }
  return undefined;
}

// 只调整未签名目录的发布坐标；插件包继续使用 CI 已验收的确切字节。
export async function retargetCatalog(directory, revision) {
  if (!Number.isInteger(revision) || revision <= 0 || revision > 0xffffffff)
    throw new Error("Invalid revision");
  const file = join(directory, "catalog-unsigned.json");
  const catalog = JSON.parse(await readFile(file, "utf8"));
  catalog.revision = revision;
  catalog.issuedAt = Math.floor(Date.now() / 1000);
  catalog.expiresAt = catalog.issuedAt + CATALOG_LIFETIME;
  for (const entry of catalog.entries) entry.releaseTag = `market-${revision}`;
  await verifyAssets(directory, catalog);
  await writeFile(file, JSON.stringify(catalog, null, 2) + "\n");
  return catalog;
}

function plan() {
  let full = true;
  if (process.env.GITHUB_EVENT_NAME === "pull_request") {
    const event = JSON.parse(
      readFileSync(process.env.GITHUB_EVENT_PATH, "utf8"),
    );
    try {
      const base = event.pull_request.base.sha,
        head = event.pull_request.head.sha;
      if (!/^[a-f\d]{40}$/i.test(base) || !/^[a-f\d]{40}$/i.test(head))
        throw new Error("Invalid revisions");
      full = requiresFullCheck(
        git(
          "diff",
          "--no-renames",
          "--name-only",
          "-z",
          `${base}...${head}`,
          "--",
        )
          .split("\0")
          .filter(Boolean),
      );
    } catch (error) {
      console.warn(
        `Unable to determine scope; running checks: ${error.message}`,
      );
    }
  }
  const tree = git("rev-parse", "HEAD^{tree}");
  let host = "",
    name = "",
    run;
  if (full) {
    const selected = process.env.SELECTED_HOST_REF;
    host = /^[a-f\d]{40}$/i.test(selected ?? "")
      ? selected.toLowerCase()
      : request(
          `repos/${readRepositoryConfig().host.repository}/commits/${encodeURIComponent(selected ?? "")}`,
        ).sha;
    if (!/^[a-f\d]{40}$/i.test(host))
      throw new Error("Unable to pin actual host SHA");
    name = checkedBuildName(tree, host);
    if (process.env.FORCE_MARKET_CHECKS !== "true")
      run = findCheckedBuild(name, {
        repository: process.env.GITHUB_REPOSITORY,
      });
  }
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    `full_required=${full}\nhost_ref=${host}\nartifact=${name}\nreused_run=${run?.id ?? ""}\n`,
  );
  const summary = run
    ? `Reusing checked market tree ${tree} / host ${host}: ${run.html_url}`
    : full
      ? `Checking market tree ${tree} / host ${host}`
      : "Only documentation, tests or CI orchestration changed; skipping native builds.";
  console.log(summary);
  if (process.env.GITHUB_STEP_SUMMARY)
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary + "\n");
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (process.argv[2] === "retarget")
    await retargetCatalog(
      resolve(process.argv[3]),
      Number(process.env.MARKET_REVISION),
    );
  else plan();
}
