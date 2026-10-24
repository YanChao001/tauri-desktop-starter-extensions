import { test } from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { publicKey, signCatalog, verifyCatalog } from "./signing.mjs";
import { packagePlugin } from "./build.mjs";
import { publishCatalog } from "./publish.mjs";

const key = () =>
  generateKeyPairSync("ed25519").privateKey.export({
    format: "pem",
    type: "pkcs8",
  });
test("signed catalogs reject payload, trusted comment and key substitution", () => {
  const privateKey = key(),
    pub = publicKey(privateKey);
  const catalog = {
    schemaVersion: 1,
    revision: 1,
    repository: "owner/repo",
    entries: [],
  };
  const signed = signCatalog(catalog, privateKey);
  assert.deepEqual(verifyCatalog(signed, pub), catalog);
  assert.throws(() =>
    verifyCatalog(
      {
        ...signed,
        payload: Buffer.from(
          JSON.stringify({ ...catalog, revision: 2 }),
        ).toString("base64"),
      },
      pub,
    ),
  );
  assert.throws(() =>
    verifyCatalog(
      {
        ...signed,
        signature: signed.signature.replace("revision:1", "revision:2"),
      },
      pub,
    ),
  );
  assert.throws(() => verifyCatalog(signed, publicKey(key())));
});
test("package identity, Wasm header and module hash are verified independently of package bytes", () => {
  const manifest = {
    formatVersion: 2,
    apiVersion: 1,
    id: "example.text",
    version: "1.0.0",
  };
  const wasm = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
  const packed = packagePlugin(manifest, wasm);
  assert.equal(packed.preview.bytes, 8);
  assert.equal(packed.preview.sha256.length, 64);
  assert.equal(JSON.parse(packed.content).manifest.id, "example.text");
  assert.throws(() => packagePlugin(manifest, Buffer.from("not wasm")));
  assert.throws(() => packagePlugin({ ...manifest, id: "../outside" }, wasm));
  assert.throws(() => packagePlugin({ ...manifest, apiVersion: 2 }, wasm));
});
test("failed uploads stay draft and older catalog revisions are not republished", () => {
  const catalog = { repository: "owner/market", revision: 2, entries: [] };
  const calls = [];
  const run = (args) => {
    calls.push(args);
    if (args[1] === "list") return JSON.stringify([{ tagName: "market-1" }]);
    if (args[1] === "upload") throw new Error("Upload failed");
    return "";
  };
  assert.throws(
    () => publishCatalog(catalog, "dist/market-2", run),
    /Upload failed/,
  );
  assert.equal(
    calls.find((args) => args[1] === "create").includes("--draft"),
    true,
  );
  assert.equal(
    calls.some((args) => args[1] === "edit"),
    false,
  );
  assert.throws(
    () => publishCatalog({ ...catalog, revision: 1 }, "dist/market-1", run),
    /must increase/,
  );
});
