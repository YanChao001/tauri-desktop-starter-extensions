import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { realpathSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { validateCatalog } from "./protocol.mjs";
import { parseJson } from "./files.mjs";

const packet = (key) => {
  if (key.asymmetricKeyType !== "ed25519")
    throw new Error("Market signing keys must use Ed25519");
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
  const parent = realpathSync(dirname(resolve(path))),
    destination = resolve(parent, basename(path)),
    source = realpathSync(resolve(import.meta.dirname, "..")),
    within = relative(source, destination);
  if (
    !within ||
    (!isAbsolute(within) &&
      within !== ".." &&
      !within.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`))
  )
    throw new Error("Private key path must be outside the repository");
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
  const envelope = {
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
  if (
    Buffer.byteLength(JSON.stringify(envelope, null, 2) + "\n") >
    2 * 1024 * 1024
  )
    throw new Error("Signed catalog exceeds 2 MiB");
  return envelope;
}

function decodeBase64(value, limit, label) {
  if (typeof value !== "string" || value.length > limit)
    throw new Error(`Invalid ${label}`);
  const decoded = Buffer.from(value, "base64");
  if (!value || decoded.toString("base64") !== value)
    throw new Error(`Noncanonical ${label} base64`);
  return decoded;
}
export function verifyCatalog(
  envelope,
  encodedKey,
  now = Math.floor(Date.now() / 1000),
) {
  if (
    !envelope ||
    typeof envelope !== "object" ||
    Array.isArray(envelope) ||
    Object.keys(envelope).some(
      (field) => !["payload", "signature"].includes(field),
    ) ||
    typeof envelope.signature !== "string" ||
    envelope.signature.length > 4096 ||
    Buffer.byteLength(JSON.stringify(envelope)) > 2 * 1024 * 1024
  )
    throw new Error("Invalid market signature envelope");
  const publicPacket = decodeBase64(encodedKey, 128, "market public key");
  const lines = envelope.signature.split("\n");
  const signature = decodeBase64(lines[1], 128, "market signature");
  const global = decodeBase64(lines[3], 128, "global signature");
  if (
    publicPacket.length !== 42 ||
    publicPacket.subarray(0, 2).toString() !== "Ed" ||
    signature.length !== 74 ||
    global.length !== 64 ||
    (lines.length !== 4 && !(lines.length === 5 && lines[4] === "")) ||
    !lines[0].startsWith("untrusted comment: ") ||
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
  const payload = decodeBase64(
    envelope.payload,
    2 * 1024 * 1024,
    "market payload",
  );
  const digest = createHash("blake2b512").update(payload).digest();
  if (
    !verify(null, digest, key, signature.subarray(10)) ||
    !verify(
      null,
      Buffer.concat([signature.subarray(10), Buffer.from(lines[2].slice(17))]),
      key,
      global,
    )
  )
    throw new Error("Invalid market signature");
  const catalog = parseJson(payload);
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
