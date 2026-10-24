import { readFile, readdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { hash, packagePlugin } from "./build.mjs";
import { verifyCatalog } from "./signing.mjs";

const directory = process.argv[2];
if (!directory)
  throw new Error("Usage: node scripts/verify.mjs <release directory>");
const root = resolve(import.meta.dirname, "..");
const catalog = verifyCatalog(
  JSON.parse(await readFile(join(directory, "catalog.json"), "utf8")),
  (await readFile(join(root, "public-key.txt"), "utf8")).trim(),
);
const assets = await readdir(directory);
if (
  catalog.schemaVersion !== 2 ||
  !Number.isInteger(catalog.revision) ||
  catalog.revision <= 0 ||
  catalog.entries.length > 100
)
  throw new Error("Invalid catalog");
const ids = new Set();
for (const entry of catalog.entries) {
  if (
    ids.has(entry.package.manifest.id) ||
    !assets.includes(entry.asset) ||
    entry.releaseTag !== `market-${catalog.revision}`
  )
    throw new Error("Invalid catalog entry");
  ids.add(entry.package.manifest.id);
  const bytes = await readFile(join(directory, entry.asset));
  if (
    bytes.length !== entry.packageBytes ||
    hash(bytes) !== entry.packageSha256
  )
    throw new Error("Package hash mismatch");
  const pkg = JSON.parse(bytes);
  const checked = packagePlugin(pkg.manifest, Buffer.from(pkg.wasm, "base64"));
  if (JSON.stringify(checked.preview) !== JSON.stringify(entry.package))
    throw new Error("Package manifest or module mismatch");
}
console.log(
  `Verified market revision ${catalog.revision}: ${ids.size} package(s)`,
);
