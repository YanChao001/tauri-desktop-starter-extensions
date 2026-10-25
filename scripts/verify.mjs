import { readFile, readdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { hash, packagePlugin } from "./build.mjs";
import { verifyCatalog } from "./signing.mjs";
import { readBounded } from "./host-tool.mjs";

const root = resolve(import.meta.dirname, "..");

/** Optional envelope/key are for an in-memory test signature, never a production trust change. */
export async function verifyRelease(directory, { envelope, encodedKey } = {}) {
  const catalog = verifyCatalog(
    envelope ??
      JSON.parse(
        (
          await readBounded(join(directory, "catalog.json"), 2 * 1024 * 1024)
        ).toString("utf8"),
      ),
    encodedKey ?? (await readFile(join(root, "public-key.txt"), "utf8")).trim(),
  );
  const assets = await readdir(directory);
  for (const entry of catalog.entries) {
    if (!assets.includes(entry.asset)) throw new Error("Missing package asset");
    const bytes = await readBounded(
      join(directory, entry.asset),
      24 * 1024 * 1024,
    );
    if (
      bytes.length !== entry.packageBytes ||
      hash(bytes) !== entry.packageSha256
    )
      throw new Error("Package hash mismatch");
    const pkg = JSON.parse(bytes);
    if (typeof pkg.wasm !== "string") throw new Error("Invalid Wasm base64");
    const module = Buffer.from(pkg.wasm, "base64");
    if (module.toString("base64") !== pkg.wasm)
      throw new Error("Noncanonical Wasm base64");
    const checked = packagePlugin(pkg.manifest, module);
    if (JSON.stringify(checked.preview) !== JSON.stringify(entry.package))
      throw new Error("Package manifest or module mismatch");
  }
  console.log(
    `Verified market revision ${catalog.revision}: ${catalog.entries.length} package(s)`,
  );
  return catalog;
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
