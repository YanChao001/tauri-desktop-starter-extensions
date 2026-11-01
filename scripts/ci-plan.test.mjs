import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import {
  requiresFullCheck,
  checkedBuildName,
  findCheckedBuild,
  retargetCatalog,
} from "./ci-plan.mjs";
import { hash, packagePlugin } from "./build.mjs";

test("unrelated documentation, tests and CI orchestration skip native builds; package inputs do not", () => {
  assert.equal(
    requiresFullCheck([
      "docs/publishing.md",
      "scripts/ci-plan.mjs",
      ".github/workflows/ci.yml",
      ".github/workflows/publish.yml",
      "scripts/ci-plan.test.mjs",
    ]),
    false,
  );
  for (const path of [
    "plugins/snake/src/lib.rs",
    "plugins/snake/ui/main.html",
    "scripts/host-tool.mjs",
    "listings.json",
    "rust-toolchain.toml",
    "unknown-input.json",
  ])
    assert.equal(requiresFullCheck([path]), true, path);
  assert.equal(requiresFullCheck([]), true);
});

test("reuse requires the exact market tree, exact host SHA and successful trusted CI", () => {
  const tree = "a".repeat(40),
    host = "b".repeat(40);
  const name = checkedBuildName(tree, host);
  const artifact = {
    name,
    expired: false,
    workflow_run: { id: 1, repository_id: 42, head_repository_id: 42 },
  };
  const success = {
    id: 1,
    path: ".github/workflows/ci.yml",
    status: "completed",
    conclusion: "success",
  };
  const lookup = (candidate = artifact, run = success) =>
    findCheckedBuild(name, {
      repository: "owner/repo",
      api: (endpoint) =>
        endpoint.includes("/artifacts?") ? { artifacts: [candidate] } : run,
    });
  assert.equal(lookup(), success);
  assert.equal(
    lookup({ ...artifact, name: checkedBuildName(tree, "c".repeat(40)) }),
    undefined,
  );
  assert.equal(
    lookup({ ...artifact, name: checkedBuildName("c".repeat(40), host) }),
    undefined,
  );
  assert.equal(lookup({ ...artifact, expired: true }), undefined);
  assert.equal(
    lookup({
      ...artifact,
      workflow_run: { ...artifact.workflow_run, head_repository_id: 99 },
    }),
    undefined,
  );
  assert.equal(
    lookup(artifact, { ...success, conclusion: "failure" }),
    undefined,
  );
  assert.equal(
    lookup(artifact, { ...success, path: ".github/workflows/publish.yml" }),
    undefined,
  );
  const warnings = [];
  assert.equal(
    findCheckedBuild(name, {
      repository: "owner/repo",
      api: () => {
        throw new Error("unavailable");
      },
      warn: (message) => warnings.push(message),
    }),
    undefined,
  );
  assert.equal(warnings.length, 1);
});

test("reused unsigned catalogs receive a new revision while preserving and verifying package bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "market-ci-reuse-"));
  try {
    const manifest = JSON.parse(
      await readFile(
        new URL(
          "../plugins/scientific-calculator/extension.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const html = await readFile(
      new URL("../plugins/scientific-calculator/ui/main.html", import.meta.url),
      "utf8",
    );
    const { content, preview } = packagePlugin(
      manifest,
      Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
      { main: html },
    );
    const asset = `${manifest.id}-${manifest.version}.starter-extension.json`;
    await writeFile(join(directory, asset), content);
    await writeFile(
      join(directory, "catalog-unsigned.json"),
      JSON.stringify({
        schemaVersion: 2,
        revision: 1,
        repository: "owner/repo",
        issuedAt: 1,
        expiresAt: 2,
        entries: [
          {
            package: preview,
            category: "tools",
            featured: false,
            releaseTag: "market-1",
            asset,
            packageSha256: hash(content),
            packageBytes: content.length,
          },
        ],
      }),
    );
    const result = await retargetCatalog(directory, 123);
    assert.equal(result.revision, 123);
    assert.equal(result.entries[0].releaseTag, "market-123");
    assert.deepEqual(await readFile(join(directory, asset)), content);
    assert.ok(result.expiresAt > Date.now() / 1000);
    await writeFile(join(directory, asset), "corrupt");
    await assert.rejects(
      retargetCatalog(directory, 124),
      /Package hash mismatch/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
