import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  rename,
  rm,
  rmdir,
  lstat,
  writeFile,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  CATALOG_LIFETIME,
  validRepository,
  validateCatalog,
  validateManifest,
  validateUiPreview,
} from "./protocol.mjs";
import {
  buildEnvironment,
  resolveHostTool,
  validatePlugin,
} from "./host-tool.mjs";
import {
  assertUnlinkedPath,
  ensureDirectory,
  parseJson,
  readBounded,
} from "./files.mjs";

const root = resolve(import.meta.dirname, "..");
export const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export function packagePlugin(manifest, wasm, uiDocuments = {}) {
  validateManifest(manifest);
  if (
    !Buffer.isBuffer(wasm) ||
    wasm.length > 16 * 1024 * 1024 ||
    !wasm.subarray(0, 8).equals(Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]))
  )
    throw new Error("Invalid Wasm module");
  if (
    !uiDocuments ||
    typeof uiDocuments !== "object" ||
    Array.isArray(uiDocuments) ||
    Object.values(uiDocuments).some(
      (value) => typeof value !== "string" || !value.isWellFormed(),
    )
  )
    throw new Error("Invalid UI documents");
  const documents = Object.entries(uiDocuments)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([id, html]) => ({
      id,
      sha256: hash(Buffer.from(html)),
      bytes: Buffer.byteLength(html),
    }));
  const preview = {
    manifest,
    sha256: hash(wasm),
    bytes: wasm.length,
    ...(documents.length ? { uiDocuments: documents } : {}),
  };
  validateUiPreview(preview);
  const content = Buffer.from(
    JSON.stringify({
      manifest,
      wasm: wasm.toString("base64"),
      ...(documents.length ? { uiDocuments } : {}),
    }) + "\n",
  );
  if (content.length > 24 * 1024 * 1024)
    throw new Error("Extension package exceeds 24 MiB");
  return {
    content,
    preview,
  };
}
export function validateListings(listings) {
  if (
    !listings ||
    typeof listings !== "object" ||
    Array.isArray(listings) ||
    Object.keys(listings).some(
      (key) => !["repository", "plugins"].includes(key),
    ) ||
    !validRepository(listings.repository) ||
    !Array.isArray(listings.plugins) ||
    listings.plugins.length > 100
  )
    throw new Error("Expected a valid repository and at most 100 listings");
  const directories = new Set();
  for (const listing of listings.plugins) {
    if (
      !listing ||
      typeof listing !== "object" ||
      Array.isArray(listing) ||
      Object.keys(listing).some(
        (key) => !["directory", "category", "featured"].includes(key),
      ) ||
      typeof listing.directory !== "string" ||
      !/^[a-z][a-z0-9-]{0,63}$/.test(listing.directory) ||
      !["text", "data", "tools", "games"].includes(listing.category) ||
      typeof listing.featured !== "boolean" ||
      directories.has(listing.directory)
    )
      throw new Error("Invalid or duplicate listing");
    directories.add(listing.directory);
  }
}
export async function build({ revision, runBuild = true }) {
  if (!Number.isInteger(revision) || revision <= 0 || revision > 0xffffffff)
    throw new Error("MARKET_REVISION must be a positive u32");
  const listings = parseJson(
    await readBounded(join(root, "listings.json"), 64 * 1024),
  );
  validateListings(listings);
  const executable = await resolveHostTool();
  const artifacts = join(root, "artifacts");
  const output = join(artifacts, `market-${revision}`);
  await ensureDirectory(artifacts);
  const lock = join(artifacts, `.market-${revision}.lock`);
  await mkdir(lock);
  let staging;
  let finished = false;
  try {
    try {
      await lstat(output);
      throw new Error(`Release directory already exists: ${output}`);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    staging = await mkdtemp(join(artifacts, `.market-${revision}-`));
    const entries = [],
      ids = new Set();
    for (const listing of listings.plugins) {
      const plugin = join(root, "plugins", listing.directory);
      await assertUnlinkedPath(plugin, true);
      const artifact = parseJson(
        await readBounded(join(plugin, "build.json"), 4096),
      ).artifact;
      if (
        typeof artifact !== "string" ||
        !/^[a-z][a-z0-9_]*\.wasm$/.test(artifact)
      )
        throw new Error("Invalid artifact name");
      const target = join(root, "target", listing.directory);
      await ensureDirectory(target);
      if (runBuild) {
        await readBounded(join(plugin, "Cargo.toml"), 1024 * 1024);
        await readBounded(join(plugin, "Cargo.lock"), 4 * 1024 * 1024);
        const formatted = spawnSync(
          "cargo",
          [
            "fmt",
            "--manifest-path",
            join(plugin, "Cargo.toml"),
            "--",
            "--check",
          ],
          {
            stdio: "inherit",
            windowsHide: true,
            cwd: root,
            env: buildEnvironment(),
          },
        );
        if (formatted.error) throw formatted.error;
        if (formatted.status !== 0)
          throw new Error(`Format check failed: ${listing.directory}`);
        const result = spawnSync(
          "cargo",
          [
            "build",
            "--locked",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            join(plugin, "Cargo.toml"),
            "--target-dir",
            target,
          ],
          {
            stdio: "inherit",
            windowsHide: true,
            cwd: root,
            env: buildEnvironment(),
          },
        );
        if (result.error) throw result.error;
        if (result.status !== 0)
          throw new Error(`Build failed: ${listing.directory}`);
      }
      // Each listing declares its artifact name; paths stay inside its isolated target directory.
      const { manifest, wasm, uiDocuments } = await validatePlugin({
        executable,
        manifestPath: join(plugin, "extension.json"),
        fixturePath: join(plugin, "smoke.json"),
        uiDirectory: join(plugin, "ui"),
        wasmPath: join(
          root,
          "target",
          listing.directory,
          "wasm32-unknown-unknown",
          "release",
          artifact,
        ),
      });
      if (ids.has(manifest.id)) throw new Error("Duplicate extension ID");
      ids.add(manifest.id);
      const { content, preview } = packagePlugin(manifest, wasm, uiDocuments);
      const asset = `${manifest.id}-${manifest.version}.starter-extension.json`;
      await writeFile(join(staging, asset), content, { flag: "wx" });
      entries.push({
        package: preview,
        category: listing.category,
        featured: listing.featured,
        releaseTag: `market-${revision}`,
        asset,
        packageSha256: hash(content),
        packageBytes: content.length,
      });
    }
    const catalog = {
      schemaVersion: 2,
      revision,
      repository: listings.repository,
      issuedAt: Math.floor(Date.now() / 1000),
      expiresAt: Math.floor(Date.now() / 1000) + CATALOG_LIFETIME,
      entries,
    };
    validateCatalog(catalog);
    await writeFile(
      join(staging, "catalog-unsigned.json"),
      JSON.stringify(catalog, null, 2) + "\n",
    );
    await rename(staging, output);
    finished = true;
    console.log(`Built ${entries.length} package(s): ${output}`);
    return output;
  } finally {
    try {
      if (!finished && staging) {
        if (
          dirname(resolve(staging)) !== resolve(artifacts) ||
          !basename(staging).startsWith(`.market-${revision}-`)
        )
          throw new Error("Unexpected staging directory");
        await rm(staging, { recursive: true });
      }
    } finally {
      await rmdir(lock);
    }
  }
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  await build({ revision: Number(process.env.MARKET_REVISION) });
}
