import { randomUUID } from "node:crypto";
import { link, rename, unlink, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { assertUnlinkedPath, readBounded } from "./files.mjs";

const limit = 2 * 1024 * 1024;

async function currentBytes(path) {
  try {
    return await readBounded(path, limit);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    return undefined;
  }
}

/** Stage complete bytes, compare planned originals, and retain backups on unsafe rollback. */
export async function applyRepositoryChanges(
  directory,
  changes,
  { beforeCommit = () => {} } = {},
) {
  directory = resolve(directory);
  await assertUnlinkedPath(directory, true);
  const staged = [],
    committed = [],
    recovery = [];
  let failure;
  async function checkParent(entry) {
    const actual = await assertUnlinkedPath(dirname(entry.target), true);
    if (actual.dev !== entry.parent.dev || actual.ino !== entry.parent.ino)
      throw new Error(
        `File directory was replaced during synchronization: ${entry.name}`,
      );
  }
  async function checkOriginal(entry) {
    const actual = await currentBytes(entry.target);
    if (
      entry.before === undefined
        ? actual !== undefined
        : !actual?.equals(entry.before)
    )
      throw new Error(
        `File was modified concurrently before synchronization: ${entry.name}`,
      );
  }
  try {
    for (const change of changes) {
      const target = resolve(directory, change.name),
        offset = relative(directory, target);
      if (
        !offset ||
        offset === ".." ||
        offset.startsWith(`..${sep}`) ||
        isAbsolute(offset)
      )
        throw new Error(
          `Synchronization path is outside the checkout: ${change.name}`,
        );
      const token = randomUUID();
      const entry = {
        ...change,
        target,
        after: Buffer.from(change.after, "utf8"),
        temporary: join(dirname(target), `.repository-sync-${token}.tmp`),
        backup: join(dirname(target), `.repository-sync-${token}.bak`),
        parent: await assertUnlinkedPath(dirname(target), true),
        moved: false,
        installed: false,
      };
      await checkOriginal(entry);
      const mode =
        entry.before === undefined
          ? 0o666
          : (await assertUnlinkedPath(target)).mode;
      staged.push(entry);
      await writeFile(entry.temporary, entry.after, { flag: "wx", mode });
    }
    for (const [index, entry] of staged.entries()) {
      await beforeCommit(index, entry);
      await checkParent(entry);
      await checkOriginal(entry);
      if (entry.before !== undefined) {
        await rename(entry.target, entry.backup);
        entry.moved = true;
      }
      committed.push(entry);
      if (
        entry.moved &&
        !(await readBounded(entry.backup, limit)).equals(entry.before)
      )
        throw new Error(
          `File was modified concurrently while replacing it: ${entry.name}`,
        );
      // Hard-link installation is atomic and fails if another writer recreates the target.
      await link(entry.temporary, entry.target);
      entry.installed = true;
    }
  } catch (error) {
    failure = error;
    for (const entry of committed.reverse()) {
      try {
        await checkParent(entry);
        if (entry.installed) {
          const actual = await currentBytes(entry.target);
          if (!actual?.equals(entry.after))
            throw new Error(
              `Rollback target was modified concurrently: ${entry.name}`,
              { cause: error },
            );
          await unlink(entry.target);
        }
        if (entry.moved) {
          // Exclusive restoration never overwrites a file created during the failed install.
          await link(entry.backup, entry.target);
          await unlink(entry.backup);
        }
      } catch (error) {
        recovery.push(
          `${entry.moved ? entry.backup : entry.target}: ${error.message}`,
        );
      }
    }
  }
  for (const entry of staged) {
    try {
      await checkParent(entry);
      await unlink(entry.temporary);
    } catch (error) {
      if (error.code !== "ENOENT")
        recovery.push(`${entry.temporary}: ${error.message}`);
    }
  }
  if (failure) {
    if (recovery.length)
      throw new Error(
        `${failure.message}\nRecovery incomplete; backups retained: ${recovery.join("; ")}`,
        { cause: failure },
      );
    throw failure;
  }
  for (const entry of staged.filter((entry) => entry.moved)) {
    try {
      await checkParent(entry);
      if (!(await readBounded(entry.backup, limit)).equals(entry.before))
        throw new Error("Original backup was modified concurrently");
      await unlink(entry.backup);
    } catch (error) {
      recovery.push(`${entry.backup}: ${error.message}`);
    }
  }
  if (recovery.length)
    console.warn(
      `Synchronization completed; recovery files retained: ${recovery.join("; ")}`,
    );
}
