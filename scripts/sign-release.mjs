import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { publicKey, signCatalog } from "./signing.mjs";
import { hash, validateListings } from "./build.mjs";
import { CATALOG_LIFETIME, validateCatalog } from "./protocol.mjs";

const directory = process.argv[2];
if (!directory)
  throw new Error("Usage: node scripts/sign-release.mjs <release directory>");
const root = resolve(import.meta.dirname, "..");
const privateKey =
  process.env.MARKET_SIGNING_PRIVATE_KEY ??
  (process.env.MARKET_KEY_FILE
    ? await readFile(process.env.MARKET_KEY_FILE, "utf8")
    : "");
if (
  !privateKey ||
  publicKey(privateKey) !==
    (await readFile(join(root, "public-key.txt"), "utf8")).trim()
)
  throw new Error("Signing key does not match public-key.txt");
const catalog = JSON.parse(
  await readFile(join(directory, "catalog-unsigned.json"), "utf8"),
);
const listings = JSON.parse(
  await readFile(join(root, "listings.json"), "utf8"),
);
validateListings(listings);
if (
  catalog.repository !== listings.repository ||
  catalog.schemaVersion !== 2 ||
  !Number.isInteger(catalog.revision) ||
  catalog.revision <= 0 ||
  catalog.revision > 0xffffffff ||
  catalog.entries.length !== listings.plugins.length
)
  throw new Error("Invalid unsigned catalog");
// 以签名时刻为有效期起点，避免构建排队缩短发布后可用时间。
catalog.issuedAt = Math.floor(Date.now() / 1000);
catalog.expiresAt = catalog.issuedAt + CATALOG_LIFETIME;
validateCatalog(catalog);
const ids = new Set();
for (let index = 0; index < catalog.entries.length; index++) {
  const entry = catalog.entries[index];
  const listing = listings.plugins[index];
  if (!/^[a-z][a-z0-9-]{0,63}$/.test(listing.directory))
    throw new Error("Invalid listing directory");
  const approved = JSON.parse(
    await readFile(
      join(root, "plugins", listing.directory, "extension.json"),
      "utf8",
    ),
  );
  if (
    ids.has(approved.id) ||
    JSON.stringify(entry.package.manifest) !== JSON.stringify(approved) ||
    entry.category !== listing.category ||
    entry.featured !== listing.featured ||
    entry.releaseTag !== `market-${catalog.revision}` ||
    entry.asset !== `${approved.id}-${approved.version}.starter-extension.json`
  )
    throw new Error("Catalog does not match reviewed listings");
  ids.add(approved.id);
  if (!/^[a-z0-9.-]+\.starter-extension\.json$/.test(entry.asset))
    throw new Error("Invalid asset path");
  const bytes = await readFile(join(directory, entry.asset));
  if (
    bytes.length !== entry.packageBytes ||
    hash(bytes) !== entry.packageSha256
  )
    throw new Error("Package changed before signing");
}
await writeFile(
  join(directory, "catalog.json"),
  JSON.stringify(signCatalog(catalog, privateKey), null, 2) + "\n",
);
console.log(`Signed market revision ${catalog.revision}`);
