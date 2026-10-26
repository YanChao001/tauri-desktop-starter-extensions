// 发布侧先检查协议和审核元数据；宿主仍在安装与运行边界重新校验。
import { isIP } from "node:net";
export const CATALOG_LIFETIME = 7 * 24 * 60 * 60;
export const MAX_CATALOG_LIFETIME = 30 * 24 * 60 * 60;
export const validRepository = (value) =>
  typeof value === "string" &&
  value.length <= 200 &&
  /^[A-Za-z0-9-]+\/[A-Za-z0-9._-]+$/.test(value) &&
  ![".", ".."].includes(value.split("/")[1]);
const hash = (value) =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const version = (value) =>
  typeof value === "string" &&
  /^(0|[1-9]\d{0,4})\.(0|[1-9]\d{0,4})\.(0|[1-9]\d{0,4})$/.test(value) &&
  value.split(".").every((part) => Number(part) <= 65535);
const nonempty = (value, limit) =>
  typeof value === "string" &&
  value.isWellFormed() &&
  value.trim().length > 0 &&
  Buffer.byteLength(value) <= limit &&
  !value.includes("\0");
const unique = (values) => new Set(values).size === values.length;
const knownFields = (value, fields) =>
  value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).every((key) => fields.includes(key));
const capabilities = new Set([
  "text:read",
  "files:read",
  "network:https",
  "network:local",
  "network:images",
  "network:system-proxy",
  "links:open",
  "storage",
  "messaging:lan",
  "wallpapers:manage",
]);
function publicHost(host) {
  const address = host.replace(/^\[|\]$/g, "");
  if (isIP(address) === 4) {
    const [a, b, c] = address.split(".").map(Number);
    return (
      ![0, 10, 127].includes(a) &&
      a < 224 &&
      !(a === 100 && b >= 64 && b <= 127) &&
      !(a === 169 && b === 254) &&
      !(a === 172 && b >= 16 && b <= 31) &&
      !(
        a === 192 &&
        ((b === 0 && [0, 2].includes(c)) || (b === 88 && c === 99) || b === 168)
      ) &&
      !(a === 198 && ([18, 19].includes(b) || (b === 51 && c === 100))) &&
      !(a === 203 && b === 0 && c === 113)
    );
  }
  if (isIP(address) === 6) {
    // 策略只依赖前三个前缀段；压缩的缺省段在这些前缀中均为零。
    const [a = 0, b = 0, c = 0] = address
      .split(":")
      .map((part) => parseInt(part || "0", 16));
    return (
      (a & 0xe000) === 0x2000 &&
      a !== 0x2002 &&
      !(
        a === 0x2001 &&
        (b === 0 ||
          (b === 2 && c === 0) ||
          b === 0x0db8 ||
          (b & 0xfff0) === 0x10 ||
          (b & 0xfff0) === 0x20)
      ) &&
      !(a === 0x3fff && (b & 0xf000) === 0)
    );
  }
  return (
    host !== "localhost" &&
    !host.endsWith(".localhost") &&
    !host.endsWith(".local") &&
    host.length <= 253 &&
    host.includes(".") &&
    host
      .split(".")
      .every(
        (label) =>
          label.length > 0 &&
          label.length <= 63 &&
          /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(label),
      )
  );
}
export function validateUiPreview(preview) {
  const expected = new Set(
    preview.manifest.commands
      .map((command) => command.ui?.document)
      .filter((id) => id != null),
  );
  const documents =
    preview.uiDocuments === undefined ? [] : preview.uiDocuments;
  if (
    !Array.isArray(documents) ||
    documents.length > 32 ||
    documents.length !== expected.size ||
    new Set(documents.map((document) => document?.id)).size !== documents.length
  )
    throw new Error("Invalid UI document preview");
  let total = 0;
  for (const document of documents) {
    if (
      !knownFields(document, ["id", "sha256", "bytes"]) ||
      !expected.has(document.id) ||
      !hash(document.sha256) ||
      !Number.isInteger(document.bytes) ||
      document.bytes < 1 ||
      document.bytes > 512 * 1024
    )
      throw new Error("Invalid UI document metadata");
    total += document.bytes;
  }
  if (total > 2 * 1024 * 1024) throw new Error("UI documents exceed 2 MiB");
}
export function validateManifest(manifest) {
  if (
    !knownFields(manifest, [
      "formatVersion",
      "apiVersion",
      "minHostVersion",
      "id",
      "version",
      "name",
      "author",
      "description",
      "icon",
      "commands",
      "permissions",
      "optionalPermissions",
      "network",
    ]) ||
    manifest.formatVersion !== 3 ||
    manifest.apiVersion !== 2 ||
    !version(manifest.minHostVersion) ||
    !version(manifest.version) ||
    typeof manifest.id !== "string" ||
    manifest.id.length > 80 ||
    manifest.id.split(".").length < 2 ||
    !manifest.id
      .split(".")
      .every((part) => /^[a-z][a-z0-9-]*$/.test(part) && !part.endsWith("-")) ||
    !nonempty(manifest.name, 120) ||
    !nonempty(manifest.author, 120) ||
    !nonempty(manifest.description, 1000) ||
    !Array.isArray(manifest.permissions) ||
    !Array.isArray(manifest.optionalPermissions) ||
    !Array.isArray(manifest.network) ||
    !Array.isArray(manifest.commands) ||
    manifest.commands.length < 1 ||
    manifest.commands.length > 32
  )
    throw new Error("Invalid extension identity or API");
  const declared = [
    ...manifest.permissions,
    ...(manifest.optionalPermissions ?? []),
  ];
  if (
    !unique(declared) ||
    declared.some((permission) => !capabilities.has(permission))
  )
    throw new Error("Invalid required or optional permissions");
  const [hostMajor, hostMinor, hostPatch] = manifest.minHostVersion
    .split(".")
    .map(Number);
  if (
    declared.some((permission) =>
      ["network:images", "network:system-proxy", "links:open"].includes(
        permission,
      ),
    ) &&
    hostMajor === 0 &&
    (hostMinor < 1 || (hostMinor === 1 && hostPatch < 6))
  )
    throw new Error(
      "Image resources, external links and system proxies require minHostVersion 0.1.6 or newer",
    );
  const ids = new Set();
  for (const command of manifest.commands) {
    if (
      !knownFields(command, [
        "id",
        "title",
        "function",
        "inputFormat",
        "resultFormat",
        "permissions",
        "ui",
      ]) ||
      !nonempty(command.id, 80) ||
      !/^[a-z0-9-]+$/.test(command.id) ||
      ids.has(command.id) ||
      !nonempty(command.title, 120) ||
      !nonempty(command.function, 128) ||
      !/^[A-Za-z0-9_.-]+$/.test(command.function) ||
      !["text", "json"].includes(command.inputFormat) ||
      !["text", "json", "markdown", "view"].includes(command.resultFormat) ||
      !Array.isArray(command.permissions) ||
      !unique(command.permissions ?? []) ||
      (command.permissions ?? []).some(
        (permission) => !declared.includes(permission),
      ) ||
      (command.resultFormat === "view" && command.inputFormat !== "json") ||
      (command.ui != null && command.resultFormat !== "view") ||
      (command.ui != null &&
        (!knownFields(command.ui, ["sidebar", "window", "document"]) ||
          typeof command.ui.sidebar !== "boolean" ||
          typeof command.ui.window !== "boolean" ||
          (command.ui.document != null &&
            (typeof command.ui.document !== "string" ||
              !/^[a-z0-9-]{1,64}$/.test(command.ui.document))))) ||
      (command.inputFormat === "text" &&
        (command.permissions ?? []).some((permission) =>
          [
            "files:read",
            "network:https",
            "network:local",
            "network:images",
            "network:system-proxy",
          ].includes(permission),
        ))
    )
      throw new Error("Invalid command contribution or permissions");
    ids.add(command.id);
  }
  if (
    manifest.icon != null &&
    ![
      "puzzle",
      "calculator",
      "gamepad",
      "notebook",
      "terminal",
      "image",
      "message-circle",
    ].includes(manifest.icon)
  )
    throw new Error("Invalid extension icon");
  const network = manifest.network === undefined ? [] : manifest.network;
  if (!Array.isArray(network) || network.length > 32)
    throw new Error("Invalid network origins");
  const origins = new Set();
  let hasLocal = false,
    hasHttps = false;
  for (const rule of network) {
    let origin;
    try {
      origin = new URL(rule.origin);
    } catch {
      throw new Error("Invalid network origin");
    }
    const local =
      origin.protocol === "http:" &&
      (origin.hostname === "localhost" ||
        origin.hostname === "[::1]" ||
        /^127\.\d+\.\d+\.\d+$/.test(origin.hostname));
    const publicHttps =
      origin.protocol === "https:" && publicHost(origin.hostname);
    if (
      !knownFields(rule, ["origin", "methods"]) ||
      origin.origin !== rule.origin ||
      origin.username ||
      origin.password ||
      origin.pathname !== "/" ||
      origin.search ||
      origin.hash ||
      (!local && !publicHttps) ||
      origins.has(rule.origin) ||
      !Array.isArray(rule.methods) ||
      !rule.methods.length ||
      !unique(rule.methods) ||
      rule.methods.some(
        (method) =>
          ![
            "GET",
            "POST",
            "PUT",
            "PATCH",
            "DELETE",
            "HEAD",
            "OPTIONS",
          ].includes(method),
      )
    )
      throw new Error("Invalid network rule");
    origins.add(rule.origin);
    hasLocal ||= local;
    hasHttps ||= publicHttps;
  }
  if (
    hasLocal !== declared.includes("network:local") ||
    hasHttps !== declared.includes("network:https")
  )
    throw new Error("Network permission does not match declared origins");
  if (
    declared.includes("network:system-proxy") &&
    !hasHttps &&
    !declared.includes("network:images")
  )
    throw new Error(
      "System proxy access requires declared public HTTP or image access",
    );
}
export function validateCatalog(catalog, now = Math.floor(Date.now() / 1000)) {
  if (
    !knownFields(catalog, [
      "schemaVersion",
      "revision",
      "repository",
      "issuedAt",
      "expiresAt",
      "entries",
    ]) ||
    catalog.schemaVersion !== 2 ||
    !validRepository(catalog.repository) ||
    !Number.isInteger(catalog.revision) ||
    catalog.revision <= 0 ||
    catalog.revision > 0xffffffff ||
    !Number.isSafeInteger(catalog.issuedAt) ||
    catalog.issuedAt <= 0 ||
    catalog.issuedAt > now + 300 ||
    !Number.isSafeInteger(catalog.expiresAt) ||
    catalog.expiresAt <= now ||
    catalog.expiresAt <= catalog.issuedAt ||
    catalog.expiresAt - catalog.issuedAt > MAX_CATALOG_LIFETIME ||
    !Array.isArray(catalog.entries) ||
    catalog.entries.length > 100
  )
    throw new Error("Invalid or expired catalog");
  const ids = new Set();
  for (const entry of catalog.entries) {
    if (
      !knownFields(entry, [
        "package",
        "category",
        "featured",
        "releaseTag",
        "asset",
        "packageSha256",
        "packageBytes",
      ]) ||
      !knownFields(entry.package, [
        "manifest",
        "sha256",
        "bytes",
        "uiDocuments",
      ])
    )
      throw new Error("Invalid catalog entry metadata");
    validateManifest(entry.package?.manifest);
    validateUiPreview(entry.package);
    if (
      ids.has(entry.package.manifest.id) ||
      !hash(entry.package.sha256) ||
      !hash(entry.packageSha256) ||
      !Number.isInteger(entry.package.bytes) ||
      entry.package.bytes < 1 ||
      entry.package.bytes > 16 * 1024 * 1024 ||
      !Number.isInteger(entry.packageBytes) ||
      entry.packageBytes < 1 ||
      entry.packageBytes > 24 * 1024 * 1024 ||
      !["text", "data", "tools", "games"].includes(entry.category) ||
      typeof entry.featured !== "boolean" ||
      entry.releaseTag !== `market-${catalog.revision}` ||
      entry.asset !==
        `${entry.package.manifest.id}-${entry.package.manifest.version}.starter-extension.json`
    )
      throw new Error("Invalid catalog entry");
    ids.add(entry.package.manifest.id);
  }
}
