import { mkdtemp, realpath, rm } from "node:fs/promises";
import { mkdtempSync, writeFileSync, rmSync, rmdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { validateCatalog, validRepository } from "./protocol.mjs";
import { verifyRelease } from "./verify.mjs";
import { parseJson, readBounded } from "./files.mjs";
import { createReleaseNotes, runGit } from "./release-notes.mjs";
import { createHosting } from "./hosting.mjs";
import { readRepositoryConfig } from "./repository-config.mjs";

const validSha = (value) =>
  typeof value === "string" &&
  value.length === 40 &&
  /^[a-f0-9]{40}$/i.test(value);

export async function publish(
  directory,
  { run, encodedKey, git = runGit, config = readRepositoryConfig() } = {},
) {
  const hosting = createHosting(config, { run });
  const root = resolve(import.meta.dirname, "..");
  const parent = await realpath(tmpdir());
  const snapshot = await mkdtemp(join(parent, "starter-market-publish-"));
  try {
    const catalog = await verifyRelease(directory, {
      encodedKey,
      snapshotDirectory: snapshot,
    });
    const repository = parseJson(
      await readBounded(join(root, "listings.json"), 64 * 1024),
    ).repository;
    if (catalog.repository !== repository)
      throw new Error("Invalid release repository");
    publishCatalog(catalog, snapshot, hosting.sourceSha(), run, {
      git,
      config,
      hosting,
    });
  } finally {
    if (
      dirname(resolve(snapshot)) !== parent ||
      !basename(snapshot).startsWith("starter-market-publish-")
    )
      throw new Error("Unexpected publication snapshot directory");
    await rm(snapshot, { recursive: true });
  }
}
export function publishCatalog(
  catalog,
  directory,
  sourceSha,
  run,
  {
    git = runGit,
    config = readRepositoryConfig(),
    hosting = createHosting(config, { run }),
  } = {},
) {
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
  const releases = hosting.listReleases(repository);
  const previous = releases.reduce(
    (highest, release) =>
      release.draft
        ? highest
        : Math.max(
            highest,
            /^market-(\d+)$/.test(release.tag_name)
              ? Number(release.tag_name.slice(7))
              : 0,
          ),
    0,
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
  hosting.verifyTagSource(repository, tag, sourceSha);
  const notes = createReleaseNotes(catalog, sourceSha, releases, git, config);
  // Published latest is changed only after every asset has been uploaded successfully.
  const notesDirectory = mkdtempSync(join(tmpdir(), "starter-market-notes-"));
  const notesFile = join(notesDirectory, "release-notes.md");
  try {
    writeFileSync(notesFile, notes, { flag: "wx", encoding: "utf8" });
    hosting.createDraft({
      repository,
      tag,
      sourceSha,
      title: `市场目录 ${catalog.revision}`,
      notesFile,
    });
  } finally {
    rmSync(notesFile, { force: true });
    rmdirSync(notesDirectory);
  }
  hosting.uploadAssets({
    repository,
    tag,
    assets: [join(directory, "catalog.json"), ...assets],
  });
  // Recheck after upload in case the remote tag was created or moved in the meantime.
  hosting.verifyTagSource(repository, tag, sourceSha);
  validateCatalog(catalog);
  hosting.publishLatest({ repository, tag });
  console.log(`Published ${repository} ${tag}`);
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (!process.argv[2] || process.argv.length !== 3)
    throw new Error(
      "Usage: node scripts/publish.mjs <verified release directory>",
    );
  await publish(resolve(process.argv[2]));
}
