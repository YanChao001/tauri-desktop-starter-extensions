import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  unlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { synchronizeRepository } from "./repository-config.mjs";
import githubRepositoryConfig from "./fixtures/github-repository.config.json" with { type: "json" };

const script = new URL("./repository-config.mjs", import.meta.url);
const original = githubRepositoryConfig;
const current = () => ({
  ...structuredClone(original),
  hosting: {
    ...structuredClone(original.hosting),
    repositoryUrlTemplate: "https://forge.example.test/projects/{repository}",
    links: {
      file: "{repositoryUrl}/source/{ref}/{path}",
      tree: "{repositoryUrl}/directory/{ref}/{path}",
      commit: "{repositoryUrl}/changes/{ref}",
      commits: "{repositoryUrl}/history/{ref}",
      compare: "{repositoryUrl}/difference/{base}/{ref}",
      release: "{repositoryUrl}/versions/{tag}",
    },
  },
  host: {
    repository: "new/host",
    repositoryUrl: "https://forge.example.test/projects/new/host",
    docsRef: "docs/preview",
    defaultRef: "integration",
  },
});
async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), "market-repository-config-"));
  t.after(async () => {
    assert.equal(dirname(directory), resolve(tmpdir()));
    assert.ok(directory.includes("market-repository-config-"));
    await rm(directory, { recursive: true });
  });
  const files = {
    "repository.config.json": JSON.stringify(original),
    "repository.generated.json": JSON.stringify({
      schemaVersion: 1,
      repository: "owner/market",
      config: original,
    }),
    "listings.json": JSON.stringify({
      repository: "owner/market",
      plugins: [],
    }),
    "README.md":
      "[host](https://github.com/owner/host)\n[docs](https://github.com/owner/host/blob/develop/docs/example.md#heading)\n[pinned](https://github.com/owner/host/blob/v1.0/docs/example.md)\n[tree](https://github.com/owner/host/tree/v1.0/sdk)\n[market](https://github.com/owner/market/releases/tag/market-1)\n[third-party](https://github.com/someone/library/blob/develop/readme.md)\n[similar](https://github.com/owner/host-extra)\n",
    ".github/actions/host-tool/action.yml":
      "runs:\n  steps:\n    - uses: actions/checkout@trusted\n      with:\n        repository: owner/host\n        ref: ${{ inputs.ref }}\n",
    ".github/workflows/ci.yml":
      "ref: ${{ vars.STARTER_HOST_REF || 'develop' }}\n",
    "plugins/wallpaper-browser/smoke.json":
      '{"headers":{"User-Agent":"Wallpaper/2.0 (https://github.com/owner/market)"}}\n',
    "plugins/wallpaper-browser/user-agent.generated.txt":
      "Wallpaper/2.0 (https://github.com/owner/market)\n",
  };
  for (const [name, value] of Object.entries(files)) {
    const path = join(directory, name);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, value);
  }
  return directory;
}
const run = (command, root) =>
  spawnSync(
    process.execPath,
    [fileURLToPath(script), command, "--root", root],
    { encoding: "utf8", windowsHide: true },
  );

test("the checkout's host integration and wallpaper fixture match its declared configuration", async () => {
  assert.deepEqual(await synchronizeRepository({ check: true }), []);
});

test("sync updates only the recorded project sources and keeps pinned refs and third-party links", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const before = await readFile(join(directory, "README.md"), "utf8");
  const drift = run("check", directory);
  assert.notEqual(drift.status, 0);
  assert.equal(await readFile(join(directory, "README.md"), "utf8"), before);
  const result = run("sync", directory);
  assert.equal(result.status, 0, result.stderr);
  const readme = await readFile(join(directory, "README.md"), "utf8");
  assert.ok(
    readme.includes(
      "https://forge.example.test/projects/new/host/source/docs%2Fpreview/docs/example.md#heading",
    ),
  );
  assert.ok(
    readme.includes(
      "https://forge.example.test/projects/new/host/source/v1.0/docs/example.md",
    ),
  );
  assert.ok(
    readme.includes(
      "https://forge.example.test/projects/new/host/directory/v1.0/sdk",
    ),
  );
  assert.ok(
    readme.includes(
      "https://forge.example.test/projects/owner/market/versions/market-1",
    ),
  );
  assert.ok(
    readme.includes(
      "https://github.com/someone/library/blob/develop/readme.md",
    ),
  );
  assert.ok(readme.includes("https://github.com/owner/host-extra"));
  assert.ok(
    (
      await readFile(
        join(directory, ".github/actions/host-tool/action.yml"),
        "utf8",
      )
    ).includes("repository: new/host"),
  );
  assert.ok(
    (
      await readFile(join(directory, ".github/workflows/ci.yml"), "utf8")
    ).includes("STARTER_HOST_REF || 'integration'"),
  );
  const smoke = JSON.parse(
    await readFile(
      join(directory, "plugins/wallpaper-browser/smoke.json"),
      "utf8",
    ),
  );
  assert.equal(
    smoke.headers["User-Agent"],
    "Wallpaper/2.0 (https://forge.example.test/projects/owner/market)",
  );
  assert.equal(
    await readFile(
      join(directory, "plugins/wallpaper-browser/user-agent.generated.txt"),
      "utf8",
    ),
    "Wallpaper/2.0 (https://forge.example.test/projects/owner/market)\n",
  );
  assert.equal(run("check", directory).status, 0);
  await writeFile(
    join(directory, ".github/actions/host-tool/action.yml"),
    "        repository: stale/host\n",
  );
  const stale = run("check", directory);
  assert.notEqual(stale.status, 0);
  assert.match(stale.stderr, /host-tool/);

  // A later migration reads the last synchronized source, rather than a baked-in GitHub prefix.
  const next = current();
  next.host.repository = "third/host";
  next.host.repositoryUrl = "https://third.example.test/third/host";
  next.host.docsRef = "next";
  next.hosting.repositoryUrlTemplate =
    "https://third.example.test/{repository}";
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(next),
  );
  assert.equal(run("sync", directory).status, 0);
  const second = await readFile(join(directory, "README.md"), "utf8");
  assert.ok(
    second.includes(
      "https://third.example.test/third/host/source/next/docs/example.md#heading",
    ),
  );
  assert.ok(
    second.includes(
      "https://third.example.test/third/host/source/v1.0/docs/example.md",
    ),
  );
  assert.ok(
    second.includes(
      "https://third.example.test/owner/market/versions/market-1",
    ),
  );
  assert.equal(run("check", directory).status, 0);
});

test("sync rejects unknown template placeholders and malicious URLs before writing", async (t) => {
  const directory = await fixture(t);
  const before = await readFile(join(directory, "README.md"), "utf8");
  for (const mutate of [
    (config) => {
      config.hosting.links.file = "{repositoryUrl}/blob/{unknown}/{path}";
    },
    (config) => {
      config.host.repositoryUrl = "javascript:alert(1)";
    },
    (config) => {
      config.host.repositoryUrl = "https://user:password@example.test/host";
    },
    (config) => {
      config.hosting.repositoryUrlTemplate =
        "https://example.test/{repository}\nInjected: value";
    },
    (config) => {
      config.wallpaperUserAgentTemplate = "Wallpaper/2.0\r\nInjected: value";
    },
    (config) => {
      config.hosting.links.file = "https://unrelated.example.test/{ref}/{path}";
    },
  ]) {
    const config = current();
    mutate(config);
    await writeFile(
      join(directory, "repository.config.json"),
      JSON.stringify(config),
    );
    const result = run("sync", directory);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Invalid|Unknown|template|HTTPS/);
    assert.equal(await readFile(join(directory, "README.md"), "utf8"), before);
  }
});

test("sync preserves inline-code delimiters around project documentation URLs", async (t) => {
  const directory = await fixture(t);
  const config = structuredClone(original);
  config.host.docsRef = "integration";
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(config),
  );
  await writeFile(
    join(directory, "README.md"),
    "Read `https://github.com/owner/host/blob/develop/docs/extensions.md` before editing.\n",
  );
  await synchronizeRepository({ directory });
  assert.equal(
    await readFile(join(directory, "README.md"), "utf8"),
    "Read `https://github.com/owner/host/blob/integration/docs/extensions.md` before editing.\n",
  );
});

test("sync preserves an editor's document change made after planning", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const path = join(directory, "README.md");
  const before = `${await readFile(path, "utf8")}Existing dirty note\n`;
  await writeFile(path, before);
  const state = await readFile(join(directory, "repository.generated.json"));
  await assert.rejects(
    synchronizeRepository({
      directory,
      beforeCommit: async (_index, entry) => {
        if (entry.name === "README.md")
          await writeFile(path, `${before}Concurrent editor note\n`);
      },
    }),
    /modified|concurrent/i,
  );
  assert.equal(
    await readFile(path, "utf8"),
    `${before}Concurrent editor note\n`,
  );
  assert.deepEqual(
    await readFile(join(directory, "repository.generated.json")),
    state,
  );
});

test("sync rolls earlier replacements back when a later document is edited concurrently", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const readme = await readFile(join(directory, "README.md"));
  const state = await readFile(join(directory, "repository.generated.json"));
  const action = ".github/actions/host-tool/action.yml";
  await assert.rejects(
    synchronizeRepository({
      directory,
      beforeCommit: async (_index, entry) => {
        if (entry.name === action)
          await writeFile(
            join(directory, action),
            "Concurrent host-tool edit\n",
          );
      },
    }),
    /modified|concurrent/i,
  );
  assert.deepEqual(await readFile(join(directory, "README.md")), readme);
  assert.equal(
    await readFile(join(directory, action), "utf8"),
    "Concurrent host-tool edit\n",
  );
  assert.deepEqual(
    await readFile(join(directory, "repository.generated.json")),
    state,
  );
});

test("sync preserves a concurrently created generated file and rolls previous files back", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const readme = await readFile(join(directory, "README.md"));
  const smokePath = join(directory, "plugins/wallpaper-browser/smoke.json");
  const smoke = await readFile(smokePath);
  const generated = "plugins/wallpaper-browser/user-agent.generated.txt";
  await unlink(join(directory, generated));
  await assert.rejects(
    synchronizeRepository({
      directory,
      beforeCommit: async (_index, entry) => {
        if (entry.name === generated)
          await writeFile(
            join(directory, generated),
            "Concurrent generated file\n",
          );
      },
    }),
    /modified|concurrent|exist/i,
  );
  assert.equal(
    await readFile(join(directory, generated), "utf8"),
    "Concurrent generated file\n",
  );
  assert.deepEqual(await readFile(join(directory, "README.md")), readme);
  assert.deepEqual(await readFile(smokePath), smoke);
});

test("sync restores original bytes after an actual staged-file installation fails", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const names = [
    "README.md",
    ".github/actions/host-tool/action.yml",
    "repository.generated.json",
  ];
  const before = await Promise.all(
    names.map((name) => readFile(join(directory, name))),
  );
  await assert.rejects(
    synchronizeRepository({
      directory,
      beforeCommit: async (_index, entry) => {
        if (entry.name === names[1]) await unlink(entry.temporary);
      },
    }),
    { code: "ENOENT" },
  );
  assert.deepEqual(
    await Promise.all(names.map((name) => readFile(join(directory, name)))),
    before,
  );
  assert.deepEqual(
    (await readdir(directory, { recursive: true })).filter((name) =>
      name.includes(".repository-sync-"),
    ),
    [],
  );
});

test("sync retains the original backup when a concurrent editor prevents safe rollback", async (t) => {
  const directory = await fixture(t);
  await writeFile(
    join(directory, "repository.config.json"),
    JSON.stringify(current()),
  );
  const path = join(directory, "README.md");
  const before = await readFile(path);
  await assert.rejects(
    synchronizeRepository({
      directory,
      beforeCommit: async (index) => {
        if (index === 1) {
          await writeFile(
            path,
            "Concurrent edit after the first replacement\n",
          );
          throw new Error("Injected next-file failure");
        }
      },
    }),
    /backups retained:.*\.bak/i,
  );
  assert.equal(
    await readFile(path, "utf8"),
    "Concurrent edit after the first replacement\n",
  );
  const backups = (await readdir(directory)).filter(
    (name) => name.includes(".repository-sync-") && name.endsWith(".bak"),
  );
  assert.equal(backups.length, 1);
  assert.deepEqual(await readFile(join(directory, backups[0])), before);
  assert.deepEqual(
    (await readdir(directory, { recursive: true })).filter((name) =>
      name.endsWith(".tmp"),
    ),
    [],
  );
});
