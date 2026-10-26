import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, rmdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const workflow = await readFile(
  new URL("../.github/workflows/publish.yml", import.meta.url),
  "utf8",
);
const metadata = workflow.match(
  /^          node --input-type=module <<'NODE'\r?\n([\s\S]*?)^          NODE\r?$/m,
)?.[1];
assert.ok(metadata, "The publish workflow must expose its metadata validation");

const hostSha = "a".repeat(40);

async function runMetadata(revision, hostRef = hostSha) {
  const directory = await mkdtemp(join(tmpdir(), "market-workflow-metadata-"));
  const outputPath = join(directory, "output");
  const environmentPath = join(directory, "environment");
  try {
    const result = spawnSync(process.execPath, ["--input-type=module"], {
      input: metadata,
      encoding: "utf8",
      env: {
        REQUESTED_REVISION: revision,
        SELECTED_HOST_REF: hostRef,
        GITHUB_OUTPUT: outputPath,
        GITHUB_ENV: environmentPath,
      },
      windowsHide: true,
    });
    if (result.error) throw result.error;
    return {
      status: result.status,
      stderr: result.stderr,
      output: await readFile(outputPath, "utf8").catch((error) => {
        if (error.code !== "ENOENT") throw error;
        return "";
      }),
      environment: await readFile(environmentPath, "utf8").catch((error) => {
        if (error.code !== "ENOENT") throw error;
        return "";
      }),
    };
  } finally {
    await rm(outputPath, { force: true });
    await rm(environmentPath, { force: true });
    await rmdir(directory);
  }
}

test("publish metadata uses the same canonical revision for output and build", async () => {
  for (const [input, expected] of [
    ["1", "1"],
    ["0007", "7"],
    ["4294967295", "4294967295"],
  ]) {
    const result = await runMetadata(input);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.output, `revision=${expected}\nhost_ref=${hostSha}\n`);
    assert.equal(result.environment, `MARKET_REVISION=${expected}\n`);
  }
});

test("publish metadata defaults an empty revision to current Unix seconds", async () => {
  const earliest = Math.floor(Date.now() / 1000);
  const result = await runMetadata("");
  const latest = Math.floor(Date.now() / 1000);
  assert.equal(result.status, 0, result.stderr);
  const revision = Number(result.output.match(/^revision=(\d+)\n/)?.[1]);
  assert.ok(revision >= earliest && revision <= latest);
  assert.equal(result.environment, `MARKET_REVISION=${revision}\n`);
});

test("publish metadata rejects nondecimal or out-of-range revisions before outputs", async () => {
  for (const input of [
    "0",
    "-1",
    "1e3",
    "0x10",
    "1.0",
    " 1",
    "1 ",
    "1\n",
    "4294967296",
    "9007199254740993",
  ]) {
    const result = await runMetadata(input);
    assert.notEqual(result.status, 0, input);
    assert.match(result.stderr, /Invalid revision/);
    assert.equal(result.output, "");
    assert.equal(result.environment, "");
  }
});

test("production metadata requires a complete immutable host commit", async () => {
  for (const input of [
    "",
    "develop",
    "v0.1.5",
    "a".repeat(39),
    `${hostSha}\n`,
  ]) {
    const result = await runMetadata("1", input);
    assert.notEqual(result.status, 0, input);
    assert.match(result.stderr, /Production host ref must be/);
    assert.equal(result.output, "");
    assert.equal(result.environment, "");
  }
});
