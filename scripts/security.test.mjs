import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { hash, packagePlugin, validateListings } from "./build.mjs";
import { ensureDirectory, parseJson, readBounded } from "./files.mjs";
import { publish, publishCatalog } from "./publish.mjs";
import { signRelease } from "./sign-release.mjs";
import {
  generateKey,
  publicKey,
  signCatalog,
  verifyCatalog,
} from "./signing.mjs";
import { verifyAssets, verifyRelease } from "./verify.mjs";

const root = resolve(import.meta.dirname, "..");
const sourceSha = "0123456789abcdef0123456789abcdef01234567";
const privateKey = () =>
  generateKeyPairSync("ed25519").privateKey.export({
    format: "pem",
    type: "pkcs8",
  });
const manifest = () => ({
  formatVersion: 3,
  apiVersion: 2,
  minHostVersion: "0.1.5",
  id: "example.test",
  version: "1.0.0",
  name: "Test",
  author: "Fixture",
  description: "Test only",
  commands: [
    {
      id: "main",
      title: "Test",
      function: "main",
      inputFormat: "json",
      resultFormat: "view",
      permissions: [],
      ui: { sidebar: true, window: true, document: "main" },
    },
  ],
  permissions: [],
  optionalPermissions: [],
  network: [],
});
const catalog = () => ({
  schemaVersion: 2,
  revision: 10,
  repository: "owner/repo",
  issuedAt: Math.floor(Date.now() / 1000),
  expiresAt: Math.floor(Date.now() / 1000) + 3600,
  entries: [],
});
async function temporary(context) {
  const parent = await realpath(tmpdir()),
    directory = await mkdtemp(join(parent, "market-security-"));
  context.after(async () => {
    assert.equal(dirname(resolve(directory)), parent);
    assert.ok(basename(directory).startsWith("market-security-"));
    await rm(directory, { recursive: true });
  });
  return directory;
}
async function releaseFixture(context) {
  const directory = await temporary(context),
    extension = manifest(),
    html = "<main>Reviewed UI 中文</main>";
  const packed = packagePlugin(
    extension,
    Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
    { main: html },
  );
  const entry = {
    package: packed.preview,
    category: "tools",
    featured: false,
    releaseTag: "market-10",
    asset: `${extension.id}-${extension.version}.starter-extension.json`,
    packageSha256: hash(packed.content),
    packageBytes: packed.content.length,
  };
  const metadata = { ...catalog(), entries: [entry] },
    key = privateKey();
  await writeFile(join(directory, entry.asset), packed.content);
  return { directory, extension, html, packed, entry, metadata, key };
}

test("signature parsing rejects malformed envelope, algorithm packets and noncanonical base64", () => {
  const key = privateKey(),
    encodedKey = publicKey(key),
    envelope = signCatalog(catalog(), key);
  const invalidKey = Buffer.from(encodedKey, "base64");
  invalidKey[0] = 0;
  for (const [candidate, trusted] of [
    [{ ...envelope, extra: true }, encodedKey],
    [{ ...envelope, payload: envelope.payload + "\n" }, encodedKey],
    [envelope, encodedKey + "\n"],
    [envelope, invalidKey.toString("base64")],
    [{ ...envelope, signature: envelope.signature + "trailing" }, encodedKey],
    [
      {
        ...envelope,
        signature: envelope.signature.replace(
          "untrusted comment: ",
          "invalid comment: ",
        ),
      },
      encodedKey,
    ],
  ])
    assert.throws(() => verifyCatalog(candidate, trusted));
  const otherKey = generateKeyPairSync("rsa", {
    modulusLength: 1024,
  }).privateKey.export({ format: "pem", type: "pkcs8" });
  assert.throws(() => publicKey(otherKey), /Ed25519/);
});

test("key generation refuses to write a private key into the market repository", () => {
  assert.throws(
    () => generateKey(join(root, "must-not-create-private-key.pem")),
    /outside the repository/,
  );
});

test("JSON input rejects malformed UTF-8 and BOM instead of silently normalizing it", () => {
  assert.throws(
    () => parseJson(Buffer.from([0x22, 0xff, 0x22])),
    /encoded data|encoding/i,
  );
  assert.throws(() =>
    parseJson(
      Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("{}")]),
    ),
  );
  assert.deepEqual(parseJson(Buffer.from('{"name":"中文"}')), { name: "中文" });
  for (const text of [
    '{"revision":1,"revision":2}',
    '{"nested":{"id":1,"\\u0069d":2}}',
  ])
    assert.throws(() => parseJson(Buffer.from(text)), /Duplicate JSON field/);
  assert.deepEqual(
    parseJson(
      Buffer.from(
        '{"a":{"id":1},"b":{"id":2},"text":"a\\\"b:{}[]:","values":[null,true,1]}',
      ),
    ),
    {
      a: { id: 1 },
      b: { id: 2 },
      text: 'a"b:{}[]:',
      values: [null, true, 1],
    },
  );
});

test("duplicate-field checks handle package-sized strings without overflowing the stack", () => {
  const content = "A".repeat(24 * 1024 * 1024 - 20);
  assert.equal(
    parseJson(Buffer.from(JSON.stringify({ wasm: content }))).wasm.length,
    content.length,
  );
});

test("bounded release reads reject oversized files, linked files and linked ancestors", async (context) => {
  const directory = await temporary(context),
    real = join(directory, "real");
  await mkdir(real);
  await writeFile(join(real, "input.json"), "12345");
  await assert.rejects(readBounded(join(real, "input.json"), 4), /at most/);
  assert.equal(
    (await readBounded(join(real, "input.json"), 5)).toString(),
    "12345",
  );
  const linked = join(directory, "linked");
  // Junctions do not require Windows developer mode or administrator privileges.
  await symlink(
    real,
    linked,
    process.platform === "win32" ? "junction" : "dir",
  );
  await assert.rejects(readBounded(join(linked, "input.json"), 5), /unlinked/);
  await assert.rejects(
    ensureDirectory(join(linked, "must-not-create")),
    /unlinked/,
  );
  await assert.rejects(readFile(join(real, "must-not-create")), /ENOENT/);
  if (process.platform !== "win32") {
    await symlink(join(real, "input.json"), join(directory, "linked.json"));
    await assert.rejects(
      readBounded(join(directory, "linked.json"), 5),
      /unlinked/,
    );
  }
});

test("package verification rejects duplicate JSON fields and null document maps", async (context) => {
  const fixture = await releaseFixture(context);
  for (const content of [
    Buffer.from(
      fixture.packed.content
        .toString()
        .replace('"manifest":', '"manifest":null,"manifest":'),
    ),
    Buffer.from(
      JSON.stringify({
        manifest: fixture.extension,
        wasm: "AGFzbQEAAAA=",
        uiDocuments: null,
      }) + "\n",
    ),
  ]) {
    fixture.entry.packageSha256 = hash(content);
    fixture.entry.packageBytes = content.length;
    await writeFile(join(fixture.directory, fixture.entry.asset), content);
    await assert.rejects(
      verifyAssets(fixture.directory, fixture.metadata),
      /Duplicate JSON field|Noncanonical|Invalid UI/,
    );
  }
});

test("production signing verifies embedded package metadata and reviewed UI before writing a signature", async (context) => {
  const fixture = await releaseFixture(context),
    repositoryRoot = join(fixture.directory, "reviewed");
  const plugin = join(repositoryRoot, "plugins", "test");
  await mkdir(join(plugin, "ui"), { recursive: true });
  await Promise.all([
    writeFile(join(repositoryRoot, "public-key.txt"), publicKey(fixture.key)),
    writeFile(
      join(repositoryRoot, "listings.json"),
      JSON.stringify({
        repository: "owner/repo",
        plugins: [{ directory: "test", category: "tools", featured: false }],
      }),
    ),
    writeFile(
      join(plugin, "extension.json"),
      JSON.stringify(fixture.extension),
    ),
    writeFile(join(plugin, "ui", "main.html"), fixture.html),
  ]);
  const assetPath = join(fixture.directory, fixture.entry.asset);
  const changed = Buffer.from(
    fixture.packed.content.toString().replace("Reviewed UI", "Tampered UI"),
  );
  fixture.entry.packageSha256 = hash(changed);
  fixture.entry.packageBytes = changed.length;
  await writeFile(assetPath, changed);
  await writeFile(
    join(fixture.directory, "catalog-unsigned.json"),
    JSON.stringify(fixture.metadata),
  );
  await assert.rejects(
    signRelease(fixture.directory, { repositoryRoot, privateKey: fixture.key }),
    /mismatch/,
  );
  const changedPackage = packagePlugin(
    fixture.extension,
    Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
    { main: fixture.html.replace("Reviewed UI", "Tampered UI") },
  );
  fixture.entry.package = changedPackage.preview;
  await writeFile(
    join(fixture.directory, "catalog-unsigned.json"),
    JSON.stringify(fixture.metadata),
  );
  await assert.rejects(
    signRelease(fixture.directory, { repositoryRoot, privateKey: fixture.key }),
    /reviewed source/,
  );
  fixture.entry.package = fixture.packed.preview;
  fixture.entry.packageSha256 = hash(fixture.packed.content);
  fixture.entry.packageBytes = fixture.packed.content.length;
  await writeFile(assetPath, fixture.packed.content);
  await writeFile(
    join(fixture.directory, "catalog-unsigned.json"),
    JSON.stringify(fixture.metadata),
  );
  await signRelease(fixture.directory, {
    repositoryRoot,
    privateKey: fixture.key,
  });
  await verifyRelease(fixture.directory, {
    encodedKey: publicKey(fixture.key),
  });
  await assert.rejects(
    signRelease(fixture.directory, { repositoryRoot, privateKey: fixture.key }),
    /EEXIST/,
  );
});

test("release history is paginated and an older high revision outside the newest page prevents writes", () => {
  const calls = [];
  assert.throws(
    () =>
      publishCatalog(catalog(), "unused", sourceSha, (args) => {
        calls.push(args);
        return JSON.stringify([
          [{ tag_name: "market-1", draft: false }],
          [{ tag_name: "market-1000", draft: false }],
        ]);
      }),
    /must increase/,
  );
  assert.equal(calls.length, 1);
  assert.ok(calls[0].includes("--paginate"));
  assert.ok(calls[0].includes("--slurp"));
  for (const response of ["{}", "[{}]", '[[{"tag_name":"market-1"}]]'])
    assert.throws(
      () => publishCatalog(catalog(), "unused", sourceSha, () => response),
      /release history/,
    );
});

test("publication uploads the verified byte snapshot even when original files change during GitHub checks", async (context) => {
  const fixture = await releaseFixture(context);
  fixture.metadata.repository = JSON.parse(
    await readFile(join(root, "listings.json"), "utf8"),
  ).repository;
  await writeFile(
    join(fixture.directory, "catalog.json"),
    JSON.stringify(signCatalog(fixture.metadata, fixture.key)),
  );
  const previous = {
    GITHUB_SHA: process.env.GITHUB_SHA,
    MARKET_SOURCE_SHA: process.env.MARKET_SOURCE_SHA,
  };
  delete process.env.GITHUB_SHA;
  process.env.MARKET_SOURCE_SHA = sourceSha;
  context.after(() => {
    for (const [name, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  });
  let snapshot;
  await publish(fixture.directory, {
    encodedKey: publicKey(fixture.key),
    run: (args) => {
      if (args[0] === "api") {
        writeFileSync(
          join(fixture.directory, fixture.entry.asset),
          "corrupted after verification",
        );
        return "[]";
      }
      if (args[1] === "upload") {
        const path = args.find((argument) =>
          argument.endsWith(fixture.entry.asset),
        );
        snapshot = dirname(path);
        assert.notEqual(snapshot, fixture.directory);
        assert.deepEqual(readFileSync(path), fixture.packed.content);
      }
      return "";
    },
  });
  await assert.rejects(readFile(join(snapshot, fixture.entry.asset)), /ENOENT/);
  const calls = [];
  await assert.rejects(
    publish(fixture.directory, {
      encodedKey: publicKey(fixture.key),
      run: (args) => calls.push(args),
    }),
    /hash mismatch/,
  );
  assert.deepEqual(calls, []);
});

test("listing metadata rejects unknown top-level fields and non-object entries", () => {
  assert.throws(() =>
    validateListings({
      repository: "owner/repo",
      plugins: [],
      unexpected: true,
    }),
  );
  assert.throws(() =>
    validateListings({ repository: "owner/repo", plugins: [null] }),
  );
  assert.throws(() =>
    packagePlugin(
      { ...manifest(), name: "\ud800" },
      Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
    ),
  );
});
