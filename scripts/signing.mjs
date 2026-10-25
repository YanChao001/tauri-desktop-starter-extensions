import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { validateCatalog } from "./protocol.mjs";

const packet = (key) => {
  const raw = createPublicKey(key)
    .export({ format: "der", type: "spki" })
    .subarray(-32);
  return Buffer.concat([
    Buffer.from("Ed"),
    createHash("sha256").update(raw).digest().subarray(0, 8),
    raw,
  ]);
};
export function publicKey(key) {
  return packet(createPrivateKey(key)).toString("base64");
}
export function generateKey(path) {
  const { privateKey } = generateKeyPairSync("ed25519");
  const pem = privateKey.export({ format: "pem", type: "pkcs8" });
  writeFileSync(path, pem, { flag: "wx", mode: 0o600 });
  return publicKey(pem);
}
export function signCatalog(
  catalog,
  privateKey,
  now = Math.floor(Date.now() / 1000),
) {
  validateCatalog(catalog, now);
  const payload = Buffer.from(JSON.stringify(catalog));
  const key = createPrivateKey(privateKey);
  const publicPacket = packet(key);
  const digest = createHash("blake2b512").update(payload).digest();
  const signature = sign(null, digest, key);
  const trusted = `market revision:${catalog.revision}`;
  const global = sign(
    null,
    Buffer.concat([signature, Buffer.from(trusted)]),
    key,
  );
  return {
    payload: payload.toString("base64"),
    signature: [
      "untrusted comment: extension market catalog",
      Buffer.concat([
        Buffer.from("ED"),
        publicPacket.subarray(2, 10),
        signature,
      ]).toString("base64"),
      `trusted comment: ${trusted}`,
      global.toString("base64"),
      "",
    ].join("\n"),
  };
}
export function verifyCatalog(
  envelope,
  encodedKey,
  now = Math.floor(Date.now() / 1000),
) {
  const publicPacket = Buffer.from(encodedKey, "base64");
  const lines = envelope.signature.split("\n");
  const signature = Buffer.from(lines[1] ?? "", "base64");
  if (
    publicPacket.length !== 42 ||
    signature.length !== 74 ||
    signature.subarray(0, 2).toString() !== "ED" ||
    !publicPacket.subarray(2, 10).equals(signature.subarray(2, 10)) ||
    !lines[2]?.startsWith("trusted comment: ")
  )
    throw new Error("Invalid market signature packet");
  const key = createPublicKey({
    key: Buffer.concat([
      Buffer.from("302a300506032b6570032100", "hex"),
      publicPacket.subarray(10),
    ]),
    format: "der",
    type: "spki",
  });
  const payload = Buffer.from(envelope.payload, "base64");
  const digest = createHash("blake2b512").update(payload).digest();
  if (
    !verify(null, digest, key, signature.subarray(10)) ||
    !verify(
      null,
      Buffer.concat([signature.subarray(10), Buffer.from(lines[2].slice(17))]),
      key,
      Buffer.from(lines[3] ?? "", "base64"),
    )
  )
    throw new Error("Invalid market signature");
  const catalog = JSON.parse(
    new TextDecoder("utf-8", { fatal: true }).decode(payload),
  );
  validateCatalog(catalog, now);
  return catalog;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (process.argv.length !== 3)
    throw new Error(
      "Usage: node scripts/signing.mjs <private key path outside repository>",
    );
  console.log(generateKey(resolve(process.argv[2])));
}
