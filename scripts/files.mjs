import { constants } from "node:fs";
import { lstat, mkdir, open } from "node:fs/promises";
import { join, parse, relative, resolve } from "node:path";

/** Reject linked ancestors as well as a linked leaf before consuming release inputs. */
export async function assertUnlinkedPath(path, directory = false) {
  const absolute = resolve(path),
    root = parse(absolute).root;
  let current = root;
  const components = relative(root, absolute).split(/[\\/]/).filter(Boolean);
  let metadata = await lstat(root);
  for (let index = 0; index < components.length; index++) {
    current = join(current, components[index]);
    metadata = await lstat(current);
    if (
      metadata.isSymbolicLink() ||
      ((index < components.length - 1 || directory) && !metadata.isDirectory())
    )
      throw new Error(
        `Expected an unlinked ${directory ? "directory" : "file"} path: ${path}`,
      );
  }
  return metadata;
}

/** Create one directory at a time, checking existing ancestors before writes. */
export async function ensureDirectory(path) {
  const absolute = resolve(path),
    root = parse(absolute).root;
  let current = root;
  for (const component of relative(root, absolute)
    .split(/[\\/]/)
    .filter(Boolean)) {
    current = join(current, component);
    let metadata;
    try {
      metadata = await lstat(current);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      try {
        await mkdir(current);
      } catch (error) {
        if (error.code !== "EEXIST") throw error;
      }
      metadata = await lstat(current);
    }
    if (metadata.isSymbolicLink() || !metadata.isDirectory())
      throw new Error(`Expected an unlinked directory path: ${path}`);
  }
}

export async function readBounded(path, limit) {
  if (!Number.isSafeInteger(limit) || limit < 0)
    throw new Error("Invalid file byte limit");
  const before = await assertUnlinkedPath(path);
  if (!before.isFile() || before.size > limit)
    throw new Error(
      `Expected a regular file of at most ${limit} bytes: ${path}`,
    );
  const file = await open(
    path,
    constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0),
  );
  try {
    const metadata = await file.stat();
    if (
      !metadata.isFile() ||
      metadata.size > limit ||
      metadata.dev !== before.dev ||
      metadata.ino !== before.ino
    )
      throw new Error(`File changed while opening: ${path}`);
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

export function parseJson(bytes) {
  // Buffer.toString() silently replaces malformed UTF-8, unlike the Rust boundary.
  const text = new TextDecoder("utf-8", {
    fatal: true,
    ignoreBOM: true,
  }).decode(bytes);
  const value = JSON.parse(text);
  // JSON.parse hides repeated fields; serde rejects them for protocol structs.
  // Syntax is already checked above, so only strings and object nesting matter.
  const stack = [];
  for (let index = 0; index < text.length; index++) {
    const current = text[index];
    if (current === "{") stack.push(new Set());
    else if (current === "[") stack.push(null);
    else if (current === "}" || current === "]") stack.pop();
    else if (current === '"') {
      const start = index++;
      // Scan iteratively: regex repetition can overflow on a 24 MiB asset string.
      while (text[index] !== '"') {
        if (text[index] === "\\") index++;
        index++;
      }
      let next = index + 1;
      while (next < text.length && text.charCodeAt(next) <= 32) next++;
      if (text[next] === ":") {
        const key = JSON.parse(text.slice(start, index + 1)),
          fields = stack.at(-1);
        if (fields.has(key)) throw new Error(`Duplicate JSON field: ${key}`);
        fields.add(key);
      }
    }
  }
  return value;
}
