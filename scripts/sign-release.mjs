import { writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { publicKey, signCatalog } from "./signing.mjs";
import { hash, validateListings } from "./build.mjs";
import { CATALOG_LIFETIME, validateCatalog } from "./protocol.mjs";
import { assertUnlinkedPath, parseJson, readBounded } from "./files.mjs";
import { verifyAssets } from "./verify.mjs";

const root = resolve(import.meta.dirname, "..");

export async function signRelease(
  directory,
  { repositoryRoot = root, privateKey } = {},
) {
  await assertUnlinkedPath(directory, true);
  const key =
    privateKey ??
    process.env.MARKET_SIGNING_PRIVATE_KEY ??
    (process.env.MARKET_KEY_FILE
      ? (await readBounded(process.env.MARKET_KEY_FILE, 16 * 1024)).toString(
          "utf8",
        )
      : "");
  if (
    !key ||
    publicKey(key) !==
      (await readBounded(join(repositoryRoot, "public-key.txt"), 1024))
        .toString("utf8")
        .trim()
  )
    throw new Error("Signing key does not match public-key.txt");
  const catalog = parseJson(
    await readBounded(
      join(directory, "catalog-unsigned.json"),
      2 * 1024 * 1024,
    ),
  );
  const listings = parseJson(
    await readBounded(join(repositoryRoot, "listings.json"), 64 * 1024),
  );
  validateListings(listings);
  if (
    !catalog ||
    typeof catalog !== "object" ||
    catalog.repository !== listings.repository ||
    !Array.isArray(catalog.entries) ||
    catalog.entries.length !== listings.plugins.length
  )
    throw new Error("Invalid unsigned catalog");
  // 以签名时刻为有效期起点，避免构建排队缩短发布后可用时间。
  catalog.issuedAt = Math.floor(Date.now() / 1000);
  catalog.expiresAt = catalog.issuedAt + CATALOG_LIFETIME;
  validateCatalog(catalog);
  const ids = new Set();
  for (let index = 0; index < catalog.entries.length; index++) {
    const entry = catalog.entries[index],
      listing = listings.plugins[index];
    const approved = parseJson(
      await readBounded(
        join(repositoryRoot, "plugins", listing.directory, "extension.json"),
        64 * 1024,
      ),
    );
    if (
      ids.has(approved.id) ||
      JSON.stringify(entry.package.manifest) !== JSON.stringify(approved) ||
      entry.category !== listing.category ||
      entry.featured !== listing.featured
    )
      throw new Error("Catalog does not match reviewed listings");
    ids.add(approved.id);
    for (const document of entry.package.uiDocuments ?? []) {
      const bytes = await readBounded(
        join(
          repositoryRoot,
          "plugins",
          listing.directory,
          "ui",
          `${document.id}.html`,
        ),
        512 * 1024,
      );
      if (bytes.length !== document.bytes || hash(bytes) !== document.sha256)
        throw new Error("Catalog UI does not match reviewed source");
    }
  }
  // A matching whole-file hash alone does not prove the embedded manifest,
  // module or UI agree with the reviewed signed preview.
  await verifyAssets(directory, catalog);
  await writeFile(
    join(directory, "catalog.json"),
    JSON.stringify(signCatalog(catalog, key), null, 2) + "\n",
    { flag: "wx" },
  );
  console.log(`Signed market revision ${catalog.revision}`);
  return catalog;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (!process.argv[2] || process.argv.length !== 3)
    throw new Error("Usage: node scripts/sign-release.mjs <release directory>");
  await signRelease(resolve(process.argv[2]));
}
