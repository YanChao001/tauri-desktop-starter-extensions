import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { verifyCatalog } from "./signing.mjs";
import { validateCatalog, validRepository } from "./protocol.mjs";

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
  publishCatalog(catalog, directory, run);
}
export function publishCatalog(catalog, directory, run) {
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
  // Published latest is changed only after every asset has been uploaded successfully.
  run([
    "release",
    "create",
    tag,
    "--repo",
    repository,
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
