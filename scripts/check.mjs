import { generateKeyPairSync } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { build } from "./build.mjs";
import { publicKey, signCatalog } from "./signing.mjs";
import { verifyRelease } from "./verify.mjs";

const directory = await build({
  revision: Number(process.env.MARKET_REVISION),
});
const catalog = JSON.parse(
  await readFile(join(directory, "catalog-unsigned.json"), "utf8"),
);
// No production key, file, or trust setting participates in the offline release regression.
const privateKey = generateKeyPairSync("ed25519").privateKey.export({
  format: "pem",
  type: "pkcs8",
});
await verifyRelease(directory, {
  envelope: signCatalog(catalog, privateKey),
  encodedKey: publicKey(privateKey),
});
console.log(
  "Offline release regression passed; no catalog.json was signed or published",
);
