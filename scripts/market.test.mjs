import { test } from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { publicKey, signCatalog, verifyCatalog } from "./signing.mjs";
import { packagePlugin } from "./build.mjs";
import { publishCatalog } from "./publish.mjs";
import {
  validateCatalog,
  validateManifest,
  MAX_CATALOG_LIFETIME,
} from "./protocol.mjs";

const key = () =>
  generateKeyPairSync("ed25519").privateKey.export({
    format: "pem",
    type: "pkcs8",
  });
const manifest = () => ({
  formatVersion: 3,
  apiVersion: 2,
  minHostVersion: "0.1.4",
  id: "example.text",
  version: "1.0.0",
  name: "Text tool",
  author: "Fixture author",
  description: "Test-only fixture",
  commands: [
    {
      id: "convert",
      title: "Convert",
      function: "convert",
      inputFormat: "text",
      resultFormat: "text",
      permissions: ["text:read"],
    },
  ],
  permissions: ["text:read"],
  optionalPermissions: [],
  network: [],
});
const emptyCatalog = (revision = 1) => ({
  schemaVersion: 2,
  revision,
  repository: "owner/repo",
  issuedAt: Math.floor(Date.now() / 1000),
  expiresAt: Math.floor(Date.now() / 1000) + 3600,
  entries: [],
});
test("signed catalogs reject payload, trusted comment and key substitution", () => {
  const privateKey = key(),
    pub = publicKey(privateKey);
  const catalog = emptyCatalog();
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
  const extension = manifest();
  const wasm = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
  const packed = packagePlugin(extension, wasm);
  assert.equal(packed.preview.bytes, 8);
  assert.equal(packed.preview.sha256.length, 64);
  assert.equal(JSON.parse(packed.content).manifest.id, "example.text");
  assert.throws(() => packagePlugin(extension, Buffer.from("not wasm")));
  assert.throws(() => packagePlugin({ ...extension, id: "../outside" }, wasm));
  assert.throws(() => packagePlugin({ ...extension, apiVersion: 1 }, wasm));
});
test("signed validity is bounded and verification refuses expired and future catalogs", () => {
  const privateKey = key(),
    pub = publicKey(privateKey);
  const catalog = emptyCatalog();
  const signed = signCatalog(catalog, privateKey);
  assert.throws(() => verifyCatalog(signed, pub, catalog.expiresAt));
  assert.throws(() => verifyCatalog(signed, pub, catalog.issuedAt - 301));
  assert.throws(() =>
    signCatalog(
      { ...catalog, expiresAt: catalog.issuedAt + MAX_CATALOG_LIFETIME + 1 },
      privateKey,
    ),
  );
  assert.throws(() => validateCatalog({ ...catalog, repository: "owner/.." }));
});
test("command formats, optional permissions and network declarations are checked before signing", () => {
  const extension = manifest();
  validateManifest(extension);
  assert.throws(() =>
    validateManifest({ ...extension, minHostVersion: "0.01.4" }),
  );
  assert.throws(() =>
    validateManifest({ ...extension, optionalPermissions: ["text:read"] }),
  );
  assert.throws(() =>
    validateManifest({
      ...extension,
      commands: [{ ...extension.commands[0], permissions: ["storage"] }],
    }),
  );
  const view = {
    ...extension.commands[0],
    inputFormat: "json",
    resultFormat: "view",
    permissions: [],
  };
  validateManifest({
    ...extension,
    permissions: [],
    optionalPermissions: ["network:https"],
    commands: [view],
    network: [{ origin: "https://api.example.com", methods: ["GET", "POST"] }],
  });
  assert.throws(() =>
    validateManifest({
      ...extension,
      network: [{ origin: "https://api.example.com/path", methods: ["GET"] }],
    }),
  );
  for (const origin of [
    "https://127.0.0.1",
    "https://10.1.2.3",
    "https://[::1]",
    "https://[2001:db8::1]",
  ]) {
    assert.throws(() =>
      validateManifest({
        ...extension,
        optionalPermissions: ["network:https"],
        network: [{ origin, methods: ["GET"] }],
      }),
    );
  }
  validateManifest({
    ...extension,
    optionalPermissions: ["network:local"],
    network: [{ origin: "http://localhost:11434", methods: ["POST"] }],
  });
});
test("failed uploads stay draft and older catalog revisions are not republished", () => {
  const catalog = { ...emptyCatalog(2), repository: "owner/market" };
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
