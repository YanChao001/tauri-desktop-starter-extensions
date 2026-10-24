import { createHash } from "node:crypto";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  CATALOG_LIFETIME,
  validRepository,
  validateCatalog,
  validateManifest,
} from "./protocol.mjs";

const root = resolve(import.meta.dirname, "..");
export const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export function packagePlugin(manifest, wasm) {
  validateManifest(manifest);
  if (
    wasm.length > 16 * 1024 * 1024 ||
    !wasm.subarray(0, 8).equals(Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]))
  )
    throw new Error("Invalid Wasm module");
  const content = Buffer.from(
    JSON.stringify({ manifest, wasm: wasm.toString("base64") }) + "\n",
  );
  if (content.length > 24 * 1024 * 1024)
    throw new Error("Extension package exceeds 24 MiB");
  return {
    content,
    preview: { manifest, sha256: hash(wasm), bytes: wasm.length },
  };
}
export async function build({ revision, runBuild = true }) {
  if (!Number.isInteger(revision) || revision <= 0 || revision > 0xffffffff)
    throw new Error("MARKET_REVISION must be a positive u32");
  const listings = JSON.parse(
    await readFile(join(root, "listings.json"), "utf8"),
  );
  if (!validRepository(listings.repository))
    throw new Error("Invalid market repository");
  const output = join(root, "dist", `market-${revision}`);
  await mkdir(join(root, "dist"), { recursive: true });
  await mkdir(output);
  const entries = [],
    ids = new Set();
  for (const listing of listings.plugins) {
    if (
      !/^[a-z][a-z0-9-]{0,63}$/.test(listing.directory) ||
      !["text", "data", "tools", "games"].includes(listing.category) ||
      typeof listing.featured !== "boolean"
    )
      throw new Error("Invalid listing");
    const plugin = join(root, "plugins", listing.directory);
    if (runBuild) {
      const env = Object.fromEntries(
        Object.entries(process.env).filter(
          ([key]) =>
            !["MARKET_SIGNING_PRIVATE_KEY", "MARKET_KEY_FILE"].includes(key),
        ),
      );
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
          join(root, "target", listing.directory),
        ],
        { stdio: "inherit", windowsHide: true, cwd: root, env },
      );
      if (result.error) throw result.error;
      if (result.status !== 0)
        throw new Error(`Build failed: ${listing.directory}`);
    }
    const manifest = JSON.parse(
      await readFile(join(plugin, "extension.json"), "utf8"),
    );
    if (ids.has(manifest.id)) throw new Error("Duplicate extension ID");
    ids.add(manifest.id);
    // Each listing declares its artifact name; paths stay inside its isolated target directory.
    const artifact = JSON.parse(
      await readFile(join(plugin, "build.json"), "utf8"),
    ).artifact;
    if (!/^[a-z][a-z0-9_]*\.wasm$/.test(artifact))
      throw new Error("Invalid artifact name");
    const wasm = await readFile(
      join(
        root,
        "target",
        listing.directory,
        "wasm32-unknown-unknown",
        "release",
        artifact,
      ),
    );
    const { content, preview } = packagePlugin(manifest, wasm);
    const asset = `${manifest.id}-${manifest.version}.starter-extension.json`;
    await writeFile(join(output, asset), content);
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
    join(output, "catalog-unsigned.json"),
    JSON.stringify(catalog, null, 2) + "\n",
  );
  console.log(`Built ${entries.length} package(s): ${output}`);
  return output;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  await build({ revision: Number(process.env.MARKET_REVISION) });
}
