import { writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { hash, packagePlugin } from "./build.mjs";
import { verifyCatalog } from "./signing.mjs";
import { assertUnlinkedPath, parseJson, readBounded } from "./files.mjs";
import { validateCatalog } from "./protocol.mjs";

const root = resolve(import.meta.dirname, "..");

/** Optional envelope/key are for an in-memory test signature, never a production trust change. */
export async function verifyRelease(
  directory,
  { envelope, encodedKey, snapshotDirectory } = {},
) {
  await assertUnlinkedPath(directory, true);
  const envelopeBytes =
    envelope === undefined
      ? await readBounded(join(directory, "catalog.json"), 2 * 1024 * 1024)
      : Buffer.from(JSON.stringify(envelope));
  const catalog = verifyCatalog(
    parseJson(envelopeBytes),
    encodedKey ??
      (await readBounded(join(root, "public-key.txt"), 1024))
        .toString("utf8")
        .trim(),
  );
  await verifyAssets(directory, catalog, { snapshotDirectory });
  if (snapshotDirectory)
    await writeFile(join(snapshotDirectory, "catalog.json"), envelopeBytes, {
      flag: "wx",
    });
  console.log(
    `Verified market revision ${catalog.revision}: ${catalog.entries.length} package(s)`,
  );
  return catalog;
}

/** Check every package before signing; a snapshot receives exactly the validated bytes. */
export async function verifyAssets(
  directory,
  catalog,
  { snapshotDirectory } = {},
) {
  validateCatalog(catalog);
  await assertUnlinkedPath(directory, true);
  if (snapshotDirectory) await assertUnlinkedPath(snapshotDirectory, true);
  for (const entry of catalog.entries) {
    const bytes = await readBounded(
      join(directory, entry.asset),
      24 * 1024 * 1024,
    );
    if (
      bytes.length !== entry.packageBytes ||
      hash(bytes) !== entry.packageSha256
    )
      throw new Error("Package hash mismatch");
    const pkg = parseJson(bytes);
    if (
      !pkg ||
      typeof pkg !== "object" ||
      Array.isArray(pkg) ||
      Object.keys(pkg).some(
        (key) => !["manifest", "wasm", "uiDocuments"].includes(key),
      )
    )
      throw new Error("Unknown package metadata");
    if (typeof pkg.wasm !== "string") throw new Error("Invalid Wasm base64");
    const module = Buffer.from(pkg.wasm, "base64");
    if (module.toString("base64") !== pkg.wasm)
      throw new Error("Noncanonical Wasm base64");
    const checked = packagePlugin(
      pkg.manifest,
      module,
      pkg.uiDocuments === undefined ? {} : pkg.uiDocuments,
    );
    if (JSON.stringify(checked.preview) !== JSON.stringify(entry.package))
      throw new Error("Package manifest or module mismatch");
    // All marketplace assets come from packagePlugin. Canonical bytes also reject
    // duplicate JSON fields that JSON.parse would hide but Rust would reject.
    if (!checked.content.equals(bytes))
      throw new Error("Noncanonical extension package");
    if (snapshotDirectory)
      await writeFile(join(snapshotDirectory, entry.asset), bytes, {
        flag: "wx",
      });
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const directory = process.argv[2];
  if (!directory || process.argv.length !== 3)
    throw new Error("Usage: node scripts/verify.mjs <release directory>");
  await verifyRelease(directory);
}
