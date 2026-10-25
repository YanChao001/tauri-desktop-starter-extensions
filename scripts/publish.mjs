import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { verifyCatalog } from "./signing.mjs";
import { validateCatalog, validRepository } from "./protocol.mjs";

const validSha = (value) =>
  typeof value === "string" &&
  value.length === 40 &&
  /^[a-f0-9]{40}$/i.test(value);

function verifyTagSource(repository, tag, sourceSha, run) {
  const expectedRef = `refs/tags/${tag}`;
  // The matching endpoint returns [] when absent, without conflating read failures with absence.
  const references = JSON.parse(
    run(["api", `repos/${repository}/git/matching-refs/tags/${tag}`]),
  );
  if (
    !Array.isArray(references) ||
    references.some(
      (entry) =>
        typeof entry?.ref !== "string" || !entry.ref.startsWith(expectedRef),
    )
  )
    throw new Error("Invalid release tag reference response");
  const matches = references.filter((entry) => entry.ref === expectedRef);
  if (matches.length > 1)
    throw new Error("Invalid duplicate release tag references");
  if (matches.length === 0) return;

  let object = matches[0].object;
  const visited = new Set();
  for (let depth = 0; depth <= 8; depth++) {
    if (!validSha(object?.sha)) throw new Error("Invalid release tag object");
    const sha = object.sha.toLowerCase();
    if (object.type === "commit") {
      if (sha !== sourceSha.toLowerCase())
        throw new Error(
          "Release tag does not match the reviewed source commit",
        );
      return;
    }
    if (object.type !== "tag" || depth === 8 || visited.has(sha))
      throw new Error("Release tag does not resolve to a commit");
    visited.add(sha);
    const annotated = JSON.parse(
      run(["api", `repos/${repository}/git/tags/${sha}`]),
    );
    if (!validSha(annotated?.sha) || annotated.sha.toLowerCase() !== sha)
      throw new Error("Invalid annotated release tag response");
    object = annotated.object;
  }
}

export function publish(
  directory,
  {
    run = (args) =>
      execFileSync("gh", args, { encoding: "utf8", windowsHide: true }),
  } = {},
) {
  const root = resolve(import.meta.dirname, "..");
  const catalog = verifyCatalog(
    JSON.parse(readFileSync(join(directory, "catalog.json"), "utf8")),
    readFileSync(join(root, "public-key.txt"), "utf8").trim(),
  );
  const repository = JSON.parse(
    readFileSync(join(root, "listings.json"), "utf8"),
  ).repository;
  if (catalog.repository !== repository)
    throw new Error("Invalid release repository");
  publishCatalog(
    catalog,
    directory,
    process.env.GITHUB_SHA ?? process.env.MARKET_SOURCE_SHA,
    run,
  );
}
export function publishCatalog(catalog, directory, sourceSha, run) {
  if (!validSha(sourceSha))
    throw new Error(
      "Release source must be a complete 40-character commit SHA",
    );
  validateCatalog(catalog);
  const repository = catalog.repository;
  if (
    !validRepository(repository) ||
    !Number.isInteger(catalog.revision) ||
    catalog.revision <= 0 ||
    catalog.revision > 0xffffffff
  )
    throw new Error("Invalid release identity");
  const releases = JSON.parse(
    run([
      "release",
      "list",
      "--repo",
      repository,
      "--exclude-drafts",
      "--limit",
      "100",
      "--json",
      "tagName",
    ]),
  );
  const previous = Math.max(
    0,
    ...releases.map(({ tagName }) =>
      /^market-(\d+)$/.test(tagName) ? Number(tagName.slice(7)) : 0,
    ),
  );
  if (catalog.revision <= previous)
    throw new Error("Market revision must increase");
  const tag = `market-${catalog.revision}`;
  const assets = catalog.entries.map((entry) => {
    if (
      entry.releaseTag !== tag ||
      !/^[a-z0-9.-]+\.starter-extension\.json$/.test(entry.asset)
    )
      throw new Error("Invalid release asset");
    return join(directory, entry.asset);
  });
  verifyTagSource(repository, tag, sourceSha, run);
  // Published latest is changed only after every asset has been uploaded successfully.
  run([
    "release",
    "create",
    tag,
    "--repo",
    repository,
    "--target",
    sourceSha,
    "--draft",
    "--title",
    `市场目录 ${catalog.revision}`,
    "--notes",
    "由维护者审核并发布的签名目录与扩展包。",
  ]);
  run([
    "release",
    "upload",
    tag,
    "--repo",
    repository,
    join(directory, "catalog.json"),
    ...assets,
  ]);
  // Recheck after upload: --target does not override a tag created or moved in the meantime.
  verifyTagSource(repository, tag, sourceSha, run);
  run([
    "release",
    "edit",
    tag,
    "--repo",
    repository,
    "--draft=false",
    "--latest",
  ]);
  console.log(`Published ${repository} ${tag}`);
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (!process.argv[2])
    throw new Error(
      "Usage: node scripts/publish.mjs <verified release directory>",
    );
  publish(resolve(process.argv[2]));
}
