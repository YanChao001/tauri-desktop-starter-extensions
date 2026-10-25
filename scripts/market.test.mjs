import { test } from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { publicKey, signCatalog, verifyCatalog } from "./signing.mjs";
import { hash, packagePlugin, validateListings } from "./build.mjs";
import { buildEnvironment, validatePlugin } from "./host-tool.mjs";
import { verifyRelease } from "./verify.mjs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
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
  for (const field of ["optionalPermissions", "network"]) {
    const missing = { ...extension };
    delete missing[field];
    assert.throws(() => validateManifest(missing));
  }
  const missingPermissions = { ...extension.commands[0] };
  delete missingPermissions.permissions;
  assert.throws(() =>
    validateManifest({ ...extension, commands: [missingPermissions] }),
  );
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
test("listings reject duplicates and paths outside plugin directories", () => {
  const listing = { directory: "text-tools", category: "text", featured: true };
  const source = { repository: "owner/market", plugins: [listing] };
  validateListings(source);
  assert.throws(() =>
    validateListings({ ...source, plugins: [listing, listing] }),
  );
  assert.throws(() =>
    validateListings({
      ...source,
      plugins: [{ ...listing, directory: "../outside" }],
    }),
  );
  assert.throws(() =>
    validateListings({ ...source, plugins: [{ ...listing, featured: "yes" }] }),
  );
  validateListings({ ...source, plugins: [] });
  assert.throws(() => validateListings({ ...source, plugins: null }));
});

async function temporary(testContext) {
  const parent = resolve(tmpdir());
  const directory = await mkdtemp(join(parent, "market-regression-"));
  testContext.after(async () => {
    assert.equal(dirname(resolve(directory)), parent);
    assert.ok(basename(directory).startsWith("market-regression-"));
    await rm(directory, { recursive: true });
  });
  return directory;
}

test("host checks use a shared byte snapshot even if source files change between validate and test", async (context) => {
  const directory = await temporary(context);
  const source = manifest();
  const wasm = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
  const paths = {
    manifestPath: join(directory, "source.json"),
    wasmPath: join(directory, "module.wasm"),
    fixturePath: join(directory, "smoke.json"),
  };
  await Promise.all([
    writeFile(paths.manifestPath, JSON.stringify(source)),
    writeFile(paths.wasmPath, wasm),
    writeFile(
      paths.fixturePath,
      JSON.stringify({
        command: "convert",
        input: "hello",
        expectOutput: "HELLO",
      }),
    ),
  ]);
  const actions = [];
  const run = (_executable, args) => {
    actions.push(args[0]);
    const checked = JSON.parse(readFileSync(args[2], "utf8"));
    assert.equal(checked.id, source.id);
    assert.deepEqual(readFileSync(args[4]), wasm);
    writeFileSync(
      paths.manifestPath,
      JSON.stringify({ ...source, id: "attacker.changed" }),
    );
    return {
      status: 0,
      stdout: JSON.stringify({
        id: source.id,
        version: source.version,
        validated: true,
        smoke: { command: "convert", passed: true },
      }),
    };
  };
  const result = await validatePlugin({
    ...paths,
    executable: "test-only-tool",
    run,
  });
  assert.deepEqual(actions, ["validate", "test"]);
  assert.equal(result.manifest.id, source.id);
  assert.deepEqual(result.wasm, wasm);
  assert.equal(
    JSON.parse(await readFile(paths.manifestPath, "utf8")).id,
    "attacker.changed",
  );
  await writeFile(
    paths.fixturePath,
    JSON.stringify({ command: "unknown", input: "", expectOutput: "" }),
  );
  await assert.rejects(
    validatePlugin({ ...paths, executable: "test-only-tool", run }),
    /does not cover command/,
  );
});

test("compilation and host processes never inherit signing credentials", () => {
  assert.deepEqual(
    buildEnvironment({
      PATH: "/tools",
      MARKET_SIGNING_PRIVATE_KEY: "secret",
      MARKET_KEY_FILE: "/private/key",
      OTHER: "kept",
    }),
    { PATH: "/tools", OTHER: "kept" },
  );
});

test("offline release verification binds signed metadata to the exact packaged module and rejects replaced assets", async (context) => {
  const directory = await temporary(context);
  const extension = manifest();
  const packed = packagePlugin(
    extension,
    Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
  );
  const asset = `${extension.id}-${extension.version}.starter-extension.json`;
  const catalog = {
    ...emptyCatalog(),
    entries: [
      {
        package: packed.preview,
        category: "text",
        featured: true,
        releaseTag: "market-1",
        asset,
        packageSha256: hash(packed.content),
        packageBytes: packed.content.length,
      },
    ],
  };
  const privateKey = key();
  const options = {
    envelope: signCatalog(catalog, privateKey),
    encodedKey: publicKey(privateKey),
  };
  await writeFile(join(directory, asset), packed.content);
  assert.deepEqual(await verifyRelease(directory, options), catalog);
  await writeFile(
    join(directory, asset),
    Buffer.from(
      packed.content.toString().replace("Fixture author", "Changed author"),
    ),
  );
  await assert.rejects(
    verifyRelease(directory, options),
    /Package hash mismatch/,
  );
});
const releaseSourceSha = "0123456789abcdef0123456789abcdef01234567";

test("publication requires a complete source SHA before contacting GitHub", () => {
  const calls = [];
  for (const sourceSha of [
    undefined,
    null,
    "",
    "main",
    "0123456",
    "g".repeat(40),
    "0".repeat(39),
    "0".repeat(41),
    `${releaseSourceSha}\n`,
  ]) {
    assert.throws(
      () =>
        publishCatalog(emptyCatalog(), "dist/market-1", sourceSha, (args) => {
          calls.push(args);
          return "[]";
        }),
      /complete 40-character commit SHA/,
    );
  }
  assert.deepEqual(calls, []);
});

test("publication binds the release tag to the exact reviewed source commit", () => {
  const calls = [];
  publishCatalog(emptyCatalog(2), "dist/market-2", releaseSourceSha, (args) => {
    calls.push(args);
    return args[0] === "api" || args[1] === "list" ? "[]" : "";
  });
  const create = calls.find((args) => args[1] === "create");
  assert.equal(create[create.indexOf("--target") + 1], releaseSourceSha);
  assert.equal(create.includes("--draft"), true);
  assert.deepEqual(
    calls.map((args) => (args[0] === "api" ? "verify-tag" : args[1])),
    ["list", "verify-tag", "create", "upload", "verify-tag", "edit"],
  );
  assert.equal(calls.at(-1).includes("--latest"), true);
});

test("failed uploads stay draft and older catalog revisions are not republished", () => {
  const catalog = { ...emptyCatalog(2), repository: "owner/market" };
  const calls = [];
  const run = (args) => {
    calls.push(args);
    if (args[0] === "api") return "[]";
    if (args[1] === "list") return JSON.stringify([{ tagName: "market-1" }]);
    if (args[1] === "upload") throw new Error("Upload failed");
    return "";
  };
  assert.throws(
    () => publishCatalog(catalog, "dist/market-2", releaseSourceSha, run),
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
    () =>
      publishCatalog(
        { ...catalog, revision: 1 },
        "dist/market-1",
        releaseSourceSha,
        run,
      ),
    /must increase/,
  );
});

const tagReference = (type, sha, tag = "market-2") => ({
  ref: `refs/tags/${tag}`,
  object: { type, sha },
});

test("publication accepts an existing lightweight tag only at the reviewed commit", () => {
  const calls = [];
  publishCatalog(emptyCatalog(2), "dist/market-2", releaseSourceSha, (args) => {
    calls.push(args);
    if (args[0] === "api")
      return JSON.stringify([tagReference("commit", releaseSourceSha)]);
    return args[1] === "list" ? "[]" : "";
  });
  assert.equal(calls.filter((args) => args[0] === "api").length, 2);
  assert.equal(calls.at(-1)[1], "edit");
});

test("a similarly prefixed tag does not occupy the intended release tag", () => {
  const calls = [];
  publishCatalog(emptyCatalog(2), "dist/market-2", releaseSourceSha, (args) => {
    calls.push(args);
    if (args[0] === "api")
      return JSON.stringify([
        tagReference("commit", "a".repeat(40), "market-20"),
      ]);
    return args[1] === "list" ? "[]" : "";
  });
  assert.equal(calls.at(-1)[1], "edit");
});

test("an existing tag at another commit is rejected before any release mutation", () => {
  const calls = [];
  assert.throws(
    () =>
      publishCatalog(
        emptyCatalog(2),
        "dist/market-2",
        releaseSourceSha,
        (args) => {
          calls.push(args);
          if (args[0] === "api")
            return JSON.stringify([tagReference("commit", "a".repeat(40))]);
          return "[]";
        },
      ),
    /does not match the reviewed source commit/,
  );
  assert.deepEqual(
    calls.map((args) => args[0]),
    ["release", "api"],
  );
  assert.equal(calls[0][1], "list");
});

test("annotated release tags are dereferenced through nested tags to the reviewed commit", () => {
  const outerSha = "a".repeat(40),
    innerSha = "b".repeat(40);
  const calls = [];
  publishCatalog(emptyCatalog(2), "dist/market-2", releaseSourceSha, (args) => {
    calls.push(args);
    if (args[0] === "api") {
      if (args[1].includes("/matching-refs/"))
        return JSON.stringify([tagReference("tag", outerSha)]);
      if (args[1].endsWith(`/${outerSha}`))
        return JSON.stringify({
          sha: outerSha,
          object: { type: "tag", sha: innerSha },
        });
      if (args[1].endsWith(`/${innerSha}`))
        return JSON.stringify({
          sha: innerSha,
          object: { type: "commit", sha: releaseSourceSha },
        });
      assert.fail("Unexpected tag lookup");
    }
    return args[1] === "list" ? "[]" : "";
  });
  assert.equal(calls.filter((args) => args[0] === "api").length, 6);
  assert.equal(calls.at(-1)[1], "edit");
});

test("failed or malformed tag reads are never treated as an absent release tag", () => {
  for (const response of [
    () => {
      throw new Error("Tag lookup unavailable");
    },
    () => "not JSON",
    () => "{}",
    () => JSON.stringify([{}]),
    () => JSON.stringify([tagReference("commit", "short")]),
    () => JSON.stringify([tagReference("blob", releaseSourceSha)]),
    () => JSON.stringify([tagReference("tag", "a".repeat(40))]),
  ]) {
    const calls = [];
    assert.throws(() =>
      publishCatalog(
        emptyCatalog(2),
        "dist/market-2",
        releaseSourceSha,
        (args) => {
          calls.push(args);
          return args[0] === "api" ? response() : "[]";
        },
      ),
    );
    assert.equal(
      calls.some((args) => ["create", "upload", "edit"].includes(args[1])),
      false,
    );
  }
});

test("incorrect, cyclic or unreadable annotated tags fail before release creation", () => {
  const annotatedSha = "a".repeat(40);
  for (const lookup of [
    () => ({
      sha: annotatedSha,
      object: { type: "commit", sha: "b".repeat(40) },
    }),
    () => ({ sha: annotatedSha, object: { type: "tag", sha: annotatedSha } }),
    () => {
      throw new Error("Annotated tag lookup unavailable");
    },
  ]) {
    const calls = [];
    assert.throws(() =>
      publishCatalog(
        emptyCatalog(2),
        "dist/market-2",
        releaseSourceSha,
        (args) => {
          calls.push(args);
          if (args[0] === "api") {
            if (args[1].includes("/matching-refs/"))
              return JSON.stringify([tagReference("tag", annotatedSha)]);
            return JSON.stringify(lookup());
          }
          return "[]";
        },
      ),
    );
    assert.equal(
      calls.some((args) => ["create", "upload", "edit"].includes(args[1])),
      false,
    );
  }
});

test("tag movement or read failure during upload prevents publication as latest", () => {
  for (const changed of [
    () => JSON.stringify([tagReference("commit", "a".repeat(40))]),
    () => {
      throw new Error("Tag lookup unavailable");
    },
  ]) {
    const calls = [];
    let reads = 0;
    assert.throws(() =>
      publishCatalog(
        emptyCatalog(2),
        "dist/market-2",
        releaseSourceSha,
        (args) => {
          calls.push(args);
          if (args[0] === "api") return ++reads === 1 ? "[]" : changed();
          return args[1] === "list" ? "[]" : "";
        },
      ),
    );
    assert.equal(
      calls.some((args) => args[1] === "create"),
      true,
    );
    assert.equal(
      calls.some((args) => args[1] === "upload"),
      true,
    );
    assert.equal(
      calls.some((args) => args[1] === "edit"),
      false,
    );
  }
});
