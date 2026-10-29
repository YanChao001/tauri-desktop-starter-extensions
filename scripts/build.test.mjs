import assert from "node:assert/strict";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";

test("an independent checkout keeps packages in artifacts and preserves caches on collision or failure", async () => {
  const parent = await realpath(tmpdir());
  const root = await mkdtemp(join(parent, "market-build-layout-"));
  const previousTool = process.env.STARTER_EXTENSION_TOOL;
  try {
    await mkdir(join(root, "scripts"));
    for (const name of [
      "build.mjs",
      "host-tool.mjs",
      "files.mjs",
      "protocol.mjs",
    ]) {
      await writeFile(
        join(root, "scripts", name),
        await readFile(new URL(name, import.meta.url)),
      );
    }
    // An empty reviewed listing exercises packaging without a compiler or host process.
    const listings = { repository: "owner/repo", plugins: [] };
    await writeFile(join(root, "listings.json"), JSON.stringify(listings));
    const tool = join(root, "extension-tool.exe");
    await writeFile(tool, "test-only placeholder; never executed");
    process.env.STARTER_EXTENSION_TOOL = tool;
    await mkdir(join(root, "target"));
    const cache = join(root, "target", "retained-cache");
    await writeFile(cache, "compiled cache");

    const { build } = await import(
      pathToFileURL(join(root, "scripts", "build.mjs"))
    );
    const output = await build({ revision: 7, runBuild: false });
    assert.equal(output, join(root, "artifacts", "market-7"));
    const catalogPath = join(output, "catalog-unsigned.json");
    const catalogBytes = await readFile(catalogPath);
    assert.equal(JSON.parse(catalogBytes).revision, 7);
    await assert.rejects(readdir(join(root, "dist")), { code: "ENOENT" });

    await assert.rejects(
      build({ revision: 7, runBuild: false }),
      /Release directory already exists/,
    );
    assert.deepEqual(await readFile(catalogPath), catalogBytes);

    // A failed later revision must remove only its lock and incomplete staging output.
    await mkdir(join(root, "plugins"));
    await mkdir(join(root, "plugins", "incomplete"));
    listings.plugins.push({
      directory: "incomplete",
      category: "tools",
      featured: false,
    });
    await writeFile(join(root, "listings.json"), JSON.stringify(listings));
    await assert.rejects(build({ revision: 8, runBuild: false }), {
      code: "ENOENT",
    });
    assert.deepEqual(await readdir(join(root, "artifacts")), ["market-7"]);
    assert.deepEqual(await readFile(catalogPath), catalogBytes);
    assert.equal(await readFile(cache, "utf8"), "compiled cache");
  } finally {
    if (previousTool === undefined) delete process.env.STARTER_EXTENSION_TOOL;
    else process.env.STARTER_EXTENSION_TOOL = previousTool;
    assert.equal(dirname(resolve(root)), parent);
    assert.ok(basename(root).startsWith("market-build-layout-"));
    await rm(root, { recursive: true });
  }
});
