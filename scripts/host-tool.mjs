import { spawnSync } from "node:child_process";
import { access, mkdtemp, open, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const limits = {
  manifest: 64 * 1024,
  wasm: 16 * 1024 * 1024,
  fixture: 1024 * 1024,
};

export function buildEnvironment(environment = process.env) {
  return Object.fromEntries(
    Object.entries(environment).filter(
      ([name]) =>
        !["MARKET_SIGNING_PRIVATE_KEY", "MARKET_KEY_FILE"].includes(name),
    ),
  );
}

export async function readBounded(path, limit) {
  const file = await open(path, "r");
  try {
    const metadata = await file.stat();
    if (!metadata.isFile() || metadata.size > limit)
      throw new Error(
        `Expected a regular file of at most ${limit} bytes: ${path}`,
      );
    const bytes = Buffer.alloc(limit + 1);
    let total = 0;
    while (total <= limit) {
      const { bytesRead } = await file.read(
        bytes,
        total,
        bytes.length - total,
        null,
      );
      if (bytesRead === 0) break;
      total += bytesRead;
    }
    if (total > limit) throw new Error(`File exceeds ${limit} bytes: ${path}`);
    return bytes.subarray(0, total);
  } finally {
    await file.close();
  }
}

export async function resolveHostTool(environment = process.env) {
  const executable = environment.STARTER_EXTENSION_TOOL
    ? environment.STARTER_EXTENSION_TOOL
    : join(
        root,
        "..",
        "src-tauri",
        "target",
        "debug",
        process.platform === "win32" ? "extension-tool.exe" : "extension-tool",
      );
  if (
    !isAbsolute(executable) ||
    !/^extension-tool(?:\.exe)?$/.test(basename(executable))
  )
    throw new Error(
      "STARTER_EXTENSION_TOOL must be the absolute path to the host extension-tool executable",
    );
  try {
    await access(executable);
  } catch {
    throw new Error(
      "Build the host extension-tool first, or set STARTER_EXTENSION_TOOL to its absolute path; see README.md",
    );
  }
  return executable;
}

function runTool(executable, action, paths, run) {
  const args = [action, "--manifest", paths.manifest, "--wasm", paths.wasm];
  if (action === "test") args.push("--fixture", paths.fixture);
  const result = run(executable, args, {
    cwd: root,
    env: buildEnvironment(),
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 8 * 1024 * 1024,
    timeout: 600_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(
      result.stderr?.trim() || result.stdout?.trim() || `Host ${action} failed`,
    );
  const report = JSON.parse(result.stdout);
  if (report.validated !== true)
    throw new Error("Host tool did not validate the extension");
  return report;
}

/** Snapshot once: packaging consumes the exact bytes checked by the authoritative Rust host. */
export async function validatePlugin({
  manifestPath,
  wasmPath,
  fixturePath,
  executable,
  run = spawnSync,
}) {
  const [manifestBytes, wasm, fixtureBytes] = await Promise.all([
    readBounded(manifestPath, limits.manifest),
    readBounded(wasmPath, limits.wasm),
    readBounded(fixturePath, limits.fixture),
  ]);
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  const fixture = JSON.parse(fixtureBytes.toString("utf8"));
  const fixtures = Array.isArray(fixture) ? fixture : [fixture];
  if (
    !Array.isArray(manifest.commands) ||
    !fixtures.length ||
    fixtures.length > 32
  )
    throw new Error(
      "Expected declared commands and between 1 and 32 smoke cases",
    );
  const covered = new Set(fixtures.map((entry) => entry?.command));
  for (const command of manifest.commands) {
    if (!covered.has(command.id))
      throw new Error(`Smoke fixture does not cover command: ${command.id}`);
  }
  const temporaryRoot = resolve(tmpdir());
  const temporary = await mkdtemp(join(temporaryRoot, "starter-market-tool-"));
  try {
    const paths = {
      manifest: join(temporary, "manifest.json"),
      wasm: join(temporary, "module.wasm"),
      fixture: join(temporary, "smoke.json"),
    };
    await Promise.all([
      writeFile(paths.manifest, manifestBytes),
      writeFile(paths.wasm, wasm),
      writeFile(paths.fixture, fixtureBytes),
    ]);
    const validated = runTool(executable, "validate", paths, run);
    const tested = runTool(executable, "test", paths, run);
    if (
      validated.id !== manifest.id ||
      validated.version !== manifest.version ||
      tested.id !== manifest.id ||
      tested.version !== manifest.version
    )
      throw new Error("Host tool returned a different extension identity");
    const reports = Array.isArray(tested.smoke) ? tested.smoke : [tested.smoke];
    if (
      reports.length !== fixtures.length ||
      reports.some((report) => report?.passed !== true)
    )
      throw new Error("Host tool did not pass all smoke cases");
    console.log(
      `Validated ${manifest.id}: ${fixtures.length} real Wasm smoke case(s)`,
    );
    return { manifest, wasm };
  } finally {
    // The target is a fresh directory created by this call, directly inside the OS temporary directory.
    if (
      dirname(resolve(temporary)) !== temporaryRoot ||
      !basename(temporary).startsWith("starter-market-tool-")
    )
      throw new Error("Unexpected temporary directory");
    await rm(temporary, { recursive: true });
  }
}
