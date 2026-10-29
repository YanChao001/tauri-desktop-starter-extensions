import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const html = await readFile(new URL("./main.html", import.meta.url), "utf8");
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const dataUrl = "data:image/jpeg;base64,AA==";
const remote = {
  resultId: "a".repeat(16),
  title: "A mountain",
  originalUrl: "https://example.com/original.jpg",
  previewUrl: "https://example.com/preview.jpg",
  sourceUrl: "https://example.com/source",
  width: 1200,
  height: 800,
};
const permissions = [
  "wallpapers:manage",
  "network:https",
  "network:images",
  "links:open",
  "network:system-proxy",
];
const resource = {
  resourceId: "test-resource",
  mime: "image/jpeg",
  bytes: 1024,
  width: 1200,
  height: 800,
  sha256: "b".repeat(64),
};
function wallpaperView(
  provider = "upx8",
  items = [remote],
  error = "",
  filters = {},
) {
  return {
    state: JSON.stringify({
      schema: 3,
      provider,
      query: filters.query ?? "",
      category: filters.category ?? (provider === "nekosbest" ? "neko" : "all"),
      page: 0,
      items,
      pending: null,
      error,
      sequence: 1,
    }),
    errors: {},
  };
}
const item = { id: "f".repeat(64), name: "Mountain", active: false, dataUrl };
const library = { revision: 1, items: [item], limit: 24 };
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
};
async function settle() {
  for (let index = 0; index < 300; index++) await Promise.resolve();
}

// This small DOM implements only the APIs used by this document. The assertions
// execute the shipped script, including its actual async handlers and renders.
async function mount({
  request,
  run,
  checkpoint,
  saved = null,
  grants = permissions,
  active = true,
  hostBusy = false,
  visibility = "visible",
  view,
  fields = {},
} = {}) {
  const documentEvents = new Map();
  const document = { activeElement: null, visibilityState: visibility };
  class Element {
    constructor(tagName) {
      this.tagName = tagName.toLowerCase();
      this.children = [];
      this.dataset = {};
      this.style = {
        setProperty(key, value) {
          this[key] = value;
        },
      };
      this.attributes = {};
      this.className = "";
      this.value = "";
      this.disabled = false;
      this.hidden = false;
      this.open = false;
    }
    append(...children) {
      for (const child of children) {
        child.parentElement = this;
        this.children.push(child);
      }
    }
    replaceChildren(...children) {
      if (this.children.some((child) => child.contains(document.activeElement)))
        document.activeElement = document.body;
      this.children = [];
      this.append(...children);
    }
    contains(target) {
      return (
        this === target || this.children.some((child) => child.contains(target))
      );
    }
    matches(selector) {
      if (selector === ":hover") return false;
      if (selector.startsWith("#")) return this.id === selector.slice(1);
      if (selector === "button[data-action]")
        return this.tagName === "button" && Boolean(this.dataset.action);
      if (selector.startsWith("."))
        return this.className.split(" ").includes(selector.slice(1));
      if (selector === '[role="tablist"]')
        return this.attributes.role === "tablist";
      return this.tagName === selector;
    }
    closest(selector) {
      return this.matches(selector)
        ? this
        : this.parentElement?.closest(selector);
    }
    querySelectorAll(selector) {
      return this.children.flatMap((child) => [
        ...(child.matches(selector) ? [child] : []),
        ...child.querySelectorAll(selector),
      ]);
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    }
    setAttribute(key, value) {
      this.attributes[key] = String(value);
      if (key === "id") this.id = String(value);
      if (key === "class") this.className = String(value);
      if (key === "title") this.title = String(value);
      if (key === "value") this.value = String(value);
      if (key === "hidden") this.hidden = true;
      if (key.startsWith("data-"))
        this.dataset[
          key.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
        ] = String(value);
    }
    getAttribute(key) {
      return this.attributes[key] ?? null;
    }
    addEventListener(name, listener) {
      this.listeners ??= new Map();
      this.listeners.set(name, listener);
    }
    focus() {
      document.activeElement = this;
    }
    getClientRects() {
      return this.hidden ||
        (this.tagName === "dialog" && !this.open) ||
        this.parentElement?.getClientRects().length === 0
        ? []
        : [{}];
    }
    click() {
      if (!this.disabled) this.onclick?.({ preventDefault() {} });
    }
    showModal() {
      this.previousFocus = document.activeElement;
      this.open = true;
    }
    close(value) {
      this.returnValue = value;
      this.open = false;
      this.previousFocus?.focus();
      this.onclose?.();
    }
    setCustomValidity(value) {
      this.validationMessage = value;
    }
    reportValidity() {
      this.validityReported = true;
      return !this.validationMessage;
    }
  }
  document.body = new Element("body");
  document.documentElement = new Element("html");
  document.activeElement = document.body;
  document.createElement = (tagName) => new Element(tagName);
  document.createElementNS = (_namespace, tagName) => new Element(tagName);
  document.querySelectorAll = (selector) =>
    document.body.querySelectorAll(selector);
  document.querySelector = (selector) => document.body.querySelector(selector);
  document.getElementById = (id) => {
    const find = (element) =>
      element.id === id ? element : element.children.map(find).find(Boolean);
    return find(document.body) ?? null;
  };
  document.addEventListener = (name, listener) =>
    documentEvents.set(name, listener);
  const markup = html
    .replace(/<(script|style)\b[^>]*>[\s\S]*?<\/\1\s*>/gi, "")
    .match(/<body\b[^>]*>([\s\S]*?)<\/body\s*>/i)[1];
  const stack = [document.body];
  const voidTags = new Set([
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "param",
    "source",
    "track",
    "wbr",
  ]);
  for (const [token, tag, attributes] of markup.matchAll(
    /<\/?([a-z][\w-]*)\b([^>]*)>/gi,
  )) {
    if (token.startsWith("</")) {
      if (stack.at(-1).tagName === tag.toLowerCase()) stack.pop();
      continue;
    }
    const element = new Element(tag);
    for (const [, name, double, single, bare] of attributes.matchAll(
      /([\w:-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g,
    ))
      element.setAttribute(name, double ?? single ?? bare ?? "");
    stack.at(-1).append(element);
    if (!voidTags.has(tag.toLowerCase()) && !token.endsWith("/>"))
      stack.push(element);
  }
  const $ = (id) => document.getElementById(id);
  for (const select of html.matchAll(
    /<select\b[^>]*\bid="([^"]+)"[^>]*>([\s\S]*?)<\/select\s*>/g,
  )) {
    const element = $(select[1]);
    element.options = Array.from(
      select[2].matchAll(/<option\b([^>]*)>([^<]*)<\/option>/g),
      (option) => ({
        value: option[1].match(/\bvalue="([^"]*)"/)?.[1] ?? option[2],
        text: option[2],
      }),
    );
    element.value = element.options[0]?.value ?? "";
  }
  const calls = [],
    runs = [],
    checkpoints = [],
    notifications = [],
    timers = new Map();
  let listener,
    timerId = 0;
  let snapshot = {
    locale: "en",
    active,
    busy: hostBusy,
    view,
    fields,
    checkpoint: saved,
    grantedPermissions: [...grants],
  };
  const starter = {
    ready: Promise.resolve(snapshot),
    onChange(next) {
      listener = next;
    },
    notify(value) {
      notifications.push(value);
      return Promise.resolve();
    },
    checkpoint(value, dirty) {
      checkpoints.push({ value, dirty });
      return checkpoint?.(value, dirty) ?? Promise.resolve();
    },
    async run(action, fields) {
      runs.push({ action, fields: structuredClone(fields) });
      const view = await (run?.(action, fields) ??
        wallpaperView(fields.provider, [remote], "", fields));
      snapshot = { ...snapshot, view };
      listener(snapshot);
      return view;
    },
    services: {
      request(service, operation) {
        calls.push({ service, ...structuredClone(operation) });
        return Promise.resolve(
          request?.(operation, service) ??
            (operation.action === "list"
              ? structuredClone(library)
              : operation.action === "fetch-image"
                ? structuredClone(resource)
                : operation.action === "preview"
                  ? { dataUrl }
                  : operation.action === "rename"
                    ? { ...library, items: [{ ...item, name: operation.name }] }
                    : { opened: true }),
        );
      },
    },
  };
  vm.runInNewContext(script, {
    document,
    window: { addEventListener() {} },
    starter,
    setTimeout(callback) {
      const id = ++timerId;
      timers.set(id, callback);
      return id;
    },
    clearTimeout(id) {
      timers.delete(id);
    },
  });
  await settle();
  return {
    $,
    document,
    calls,
    runs,
    checkpoints,
    notifications,
    setVisibility(value) {
      document.visibilityState = value;
      documentEvents.get("visibilitychange")?.();
    },
    update(value) {
      snapshot = { ...snapshot, ...value };
      listener(snapshot);
    },
    async flushTimers() {
      const pending = [...timers.values()];
      timers.clear();
      pending.forEach((callback) => callback());
      await settle();
    },
  };
}

function galleryAction(app, action, wallpaperId = item.id) {
  const card = app
    .$("gallery")
    .children.find((node) => node.dataset.wallpaperId === wallpaperId);
  return card
    .querySelectorAll("button[data-action]")
    .find((node) => node.dataset.action === action);
}

test("source filters, tab and fit restore while typing only checkpoints the search draft", async () => {
  const app = await mount();
  assert.deepEqual(app.runs, [
    {
      action: "load",
      fields: { provider: "upx8", query: "", category: "all" },
    },
  ]);
  app.$("provider").value = "wallhaven";
  app.$("provider").onchange();
  await settle();
  const beforeTyping = app.runs.length;
  app.$("query").value = "mountain";
  app.$("query").oninput();
  await app.flushTimers();
  assert.equal(app.runs.length, beforeTyping);
  app.$("fit").value = "contain";
  app.$("fit").onchange();
  await settle();
  const saved = app.checkpoints.at(-1);
  assert.equal(saved.dirty, false);
  assert.deepEqual(JSON.parse(JSON.stringify(saved.value)), {
    provider: "wallhaven",
    query: "mountain",
    category: "all",
    tab: "remote",
    fit: "contain",
  });
  const reopened = await mount({
    saved: { ...saved.value, networkMode: "direct" },
  });
  assert.equal(reopened.$("provider").value, "wallhaven");
  assert.deepEqual(reopened.runs, [
    {
      action: "load",
      fields: { provider: "wallhaven", query: "mountain", category: "all" },
    },
  ]);
  assert.equal(reopened.$("network-mode"), null);
  assert.equal(reopened.$("query").value, "mountain");
  assert.equal(reopened.$("category").value, "all");
  assert.equal(reopened.$("fit").value, "contain");
});

test("a restored validated collection renders thumbnails without requesting another API batch", async () => {
  const app = await mount({
    view: wallpaperView("wallhaven"),
    saved: {
      provider: "wallhaven",
      query: "",
      category: "all",
      tab: "remote",
      fit: "cover",
    },
  });
  assert.equal(app.runs.length, 0);
  assert.equal(app.$("provider").value, "wallhaven");
  assert.equal(app.$("results").querySelector("img")?.src, dataUrl);
  assert.equal(
    app.calls.filter((operation) => operation.action === "list").length,
    1,
  );
});

test("category selection loads real filters and Enter submits the query without reacting to composition", async () => {
  const app = await mount({
    saved: { provider: "wallhaven", query: "", category: "all" },
  });
  app.$("category").value = "anime";
  app.$("category").onchange();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "wallhaven", query: "", category: "anime" },
  });
  app.$("query").value = "blue sky";
  app.$("query").oninput();
  const beforeEnter = app.runs.length;
  app
    .$("query")
    .onkeydown({ key: "Enter", isComposing: true, preventDefault() {} });
  app.$("query").onkeydown({ key: "Enter", repeat: true, preventDefault() {} });
  await settle();
  assert.equal(app.runs.length, beforeEnter);
  app.$("query").onkeydown({ key: "Enter", preventDefault() {} });
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "wallhaven", query: "blue sky", category: "anime" },
  });
});

test("UPX8 text search selects its API search mode and leaves the typed draft intact", async () => {
  const app = await mount({
    saved: { provider: "upx8", query: "", category: "nature" },
  });
  app.$("query").value = "雪山";
  app.$("query").oninput();
  assert.equal(app.$("category").disabled, true);
  app.$("search").click();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "upx8", query: "雪山", category: "all" },
  });
  assert.equal(app.$("query").value, "雪山");
  app.$("query").value = "";
  app.$("query").oninput();
  assert.equal(app.$("category").disabled, false);
});

test("invalid query drafts never dispatch a load or replace the last valid collection", async () => {
  const app = await mount();
  const initialLoads = app.runs.length;
  for (const query of ["bad\u0000query", "山".repeat(101)]) {
    app.$("query").value = query;
    app.$("query").oninput();
    app.$("search").click();
    await settle();
    assert.equal(app.runs.length, initialLoads);
    assert.equal(app.$("query").value, query);
    assert.equal(
      app.$("results").children[0].dataset.resultId,
      remote.resultId,
    );
  }
});

test("a pending search preserves the next query draft and focus across late results and locale changes", async () => {
  let response = null;
  const app = await mount({
    saved: { provider: "wallhaven", query: "", category: "general" },
    run: (_action, fields) =>
      response?.promise ?? wallpaperView(fields.provider, [remote], "", fields),
  });
  response = deferred();
  app.$("query").value = "first mountain";
  app.$("query").oninput();
  app.$("query").focus();
  app.$("query").onkeydown({ key: "Enter", preventDefault() {} });
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: {
      provider: "wallhaven",
      query: "first mountain",
      category: "general",
    },
  });
  app.update({ busy: true });
  const savedBeforeDraft = app.checkpoints.length;
  assert.equal(app.$("query").disabled, false);
  assert.equal(app.$("search").disabled, true);
  app.$("query").value = "next ocean";
  app.$("query").oninput();
  app.$("query").onkeydown({ key: "Enter", preventDefault() {} });
  app.update({ locale: "zh-CN" });
  await app.flushTimers();
  assert.equal(
    app.runs.length,
    2,
    "a pending request cannot submit a second search",
  );
  assert.equal(
    app.checkpoints.length,
    savedBeforeDraft,
    "the host stays free of draft requests while busy",
  );
  response.resolve(
    wallpaperView("wallhaven", [remote], "", {
      query: "first mountain",
      category: "general",
    }),
  );
  await settle();
  assert.equal(app.checkpoints.length, savedBeforeDraft);
  app.update({ busy: false });
  await settle();
  assert.equal(app.checkpoints.at(-1).value.query, "next ocean");
  assert.equal(app.$("query").value, "next ocean");
  assert.equal(app.document.activeElement, app.$("query"));
  assert.equal(app.$("category").value, "general");
  assert.equal(app.$("search").disabled, false);
  assert.equal(
    app.runs.length,
    2,
    "receiving a result does not auto-submit the new draft",
  );
});

test("a draft timer deferred by leaving the page saves the same query when the page returns", async () => {
  const app = await mount();
  const saved = app.checkpoints.length;
  app.$("query").value = "typed before leaving";
  app.$("query").oninput();
  app.update({ active: false });
  await app.flushTimers();
  assert.equal(app.checkpoints.length, saved);
  app.update({ active: true });
  await settle();
  assert.equal(app.checkpoints.at(-1).value.query, "typed before leaving");
  assert.equal(app.runs.length, 1);
});

test("changing source selects its supported category and exposes the correct search meaning", async () => {
  const app = await mount({
    saved: { provider: "wallhaven", category: "people", query: "portrait" },
  });
  app.$("provider").value = "nekosbest";
  app.$("provider").onchange();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "nekosbest", query: "", category: "neko" },
  });
  assert.match(app.$("query-label").textContent, /creator|artwork/i);
  app.$("category").value = "kitsune";
  app.$("category").onchange();
  await settle();
  app.$("query").value = "artist name";
  app.$("query").oninput();
  app.$("search").click();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: {
      provider: "nekosbest",
      query: "artist name",
      category: "kitsune",
    },
  });
  app.$("provider").value = "commons";
  app.$("provider").onchange();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "commons", query: "", category: "all" },
  });
  assert.match(app.$("category-label").textContent, /theme/i);
});

for (const proxyGranted of [false, true])
  test(`provider loads and image resources inherit host connection settings${proxyGranted ? " with the proxy grant" : " without the proxy grant"}`, async () => {
    const app = await mount({
      grants: permissions.filter(
        (permission) => proxyGranted || permission !== "network:system-proxy",
      ),
      saved: { provider: "nekosbest", networkMode: "system" },
      request: (operation) =>
        operation.action === "save" ? { cancelled: false } : undefined,
    });
    assert.deepEqual(app.runs, [
      {
        action: "load",
        fields: { provider: "nekosbest", query: "", category: "neko" },
      },
    ]);
    assert.equal("networkMode" in app.checkpoints.at(-1).value, false);
    app.$("results").children[0].click();
    await settle();
    app
      .$("preview")
      .querySelectorAll("button[data-action]")
      .find((button) => button.dataset.action === "download-original")
      .click();
    await settle();
    assert.deepEqual(
      app.calls.filter((operation) => operation.action === "fetch-image"),
      [
        { service: "resources", action: "fetch-image", url: remote.previewUrl },
        {
          service: "resources",
          action: "fetch-image",
          url: remote.originalUrl,
        },
      ],
    );
  });

test("switching to the gallery during a provider load refreshes after the active operation", async () => {
  const load = deferred();
  const app = await mount({
    run: () => load.promise,
  });
  app.$("local-tab").click();
  assert.equal(app.calls.filter((call) => call.action === "list").length, 1);
  load.resolve(wallpaperView());
  await settle();
  assert.equal(app.calls.filter((call) => call.action === "list").length, 2);
  assert.equal(app.$("local-panel").hidden, false);
  assert.equal(app.$("refresh").disabled, false);
});

test("environment renders preserve focused results, gallery actions and image details", async () => {
  const app = await mount();
  let result = app.$("results").children[0];
  result.focus();
  app.update({ locale: "zh-CN" });
  assert.equal(app.document.activeElement.dataset.resultId, remote.resultId);
  result = app.$("results").children[0];
  result.click();
  await settle();
  app.$("preview-info").focus();
  app.$("preview-info").click();
  app.$("image-info-close").focus();
  app.update({ locale: "en" });
  assert.equal(app.$("image-info-dialog").open, true);
  assert.equal(app.document.activeElement.id, "image-info-close");
  app.$("image-info-close").click();
  assert.equal(app.$("preview-dialog").open, true);
  assert.equal(app.document.activeElement, app.$("preview-info"));
  app.$("preview-close").click();
  app.$("local-tab").click();
  await settle();
  const rename = galleryAction(app, "rename");
  rename.focus();
  app.update({ locale: "zh-CN" });
  assert.equal(app.document.activeElement.dataset.action, "rename");
  assert.equal(
    app.document.activeElement.closest(".card").dataset.wallpaperId,
    item.id,
  );
});

test("image information survives a late thumbnail and closes with the preview or revoked access", async () => {
  const preview = deferred();
  const app = await mount({
    request: (operation, service) =>
      service === "resources" && operation.action === "preview"
        ? preview.promise
        : undefined,
  });
  app.$("results").children[0].click();
  app.$("preview-info").focus();
  app.$("preview-info").click();
  app.$("image-info-close").focus();
  assert.equal(app.$("image-info-dialog").open, true);
  preview.resolve({ dataUrl });
  await settle();
  assert.equal(app.$("preview-image").src, dataUrl);
  assert.equal(app.$("image-info-dialog").open, true);
  assert.equal(app.document.activeElement, app.$("image-info-close"));
  app.$("preview-dialog").close();
  assert.equal(app.$("image-info-dialog").open, false);
  app.$("results").children[0].click();
  app.$("preview-info").focus();
  app.$("preview-info").click();
  app.update({ grantedPermissions: [] });
  assert.equal(app.$("image-info-dialog").open, false);
  assert.equal(app.$("preview-dialog").open, false);
});

test("preview actions use short icon labels and import the original through a released resource", async () => {
  const imported = { ...item, id: "e".repeat(64), name: remote.title };
  const app = await mount({
    request: (operation) =>
      operation.action === "import-resource"
        ? { library: { ...library, revision: 2, items: [item, imported] } }
        : undefined,
  });
  app.$("results").children[0].click();
  for (const [id, label] of [
    ["add-to-gallery", "Add to gallery"],
    ["download-original", "Save original"],
    ["open-source", "Source"],
  ]) {
    const action = app.$(id);
    const visibleLabel = action.querySelector(".action-label").textContent;
    assert.equal(visibleLabel, label);
    assert.doesNotMatch(visibleLabel, /…|\.\.\./);
    assert.ok(action.querySelector("svg"));
  }
  app.$("add-to-gallery").click();
  await settle();
  const importedRequest = app.calls.find(
    (operation) => operation.action === "import-resource",
  );
  assert.equal(importedRequest.resourceId, resource.resourceId);
  assert.equal(importedRequest.name, remote.title);
  assert.equal(
    app.calls.filter((operation) => operation.action === "fetch-image").at(-1)
      .url,
    remote.originalUrl,
  );
  assert.equal(
    app.calls.filter((operation) => operation.action === "release").length,
    2,
  );
  assert.equal(app.$("gallery").querySelectorAll(".card").length, 2);
});

test("a load invalidated by HTTPS revocation cannot return through environment snapshots or regrant", async () => {
  let pending = null;
  let items = [remote];
  const other = {
    ...remote,
    resultId: "c".repeat(16),
    title: "New collection",
    previewUrl: "https://example.com/new-preview.jpg",
  };
  const app = await mount({
    run: (_action, fields) =>
      pending?.promise ?? wallpaperView(fields.provider, items, "", fields),
  });
  const expectOriginalCollection = () => {
    assert.equal(
      app.$("results").children[0].dataset.resultId,
      remote.resultId,
    );
    assert.equal(
      app.calls.filter(
        (operation) =>
          operation.action === "fetch-image" &&
          operation.url === other.previewUrl,
      ).length,
      0,
    );
  };
  pending = deferred();
  app.$("query").value = "new search";
  app.$("query").oninput();
  app.$("search").click();
  await settle();
  app.update({
    grantedPermissions: permissions.filter(
      (permission) => permission !== "network:https",
    ),
  });
  pending.resolve(
    wallpaperView("upx8", [other], "", {
      query: "new search",
      category: "all",
    }),
  );
  await settle();
  expectOriginalCollection();
  for (const environment of [
    { locale: "zh-CN" },
    { active: false },
    { active: true },
  ]) {
    app.update(environment);
    await settle();
    expectOriginalCollection();
  }
  app.update({ grantedPermissions: permissions });
  await settle();
  expectOriginalCollection();
  assert.equal(
    app.runs.length,
    2,
    "regrant does not restart the discarded request",
  );
  pending = null;
  items = [other];
  app.$("search").click();
  await settle();
  assert.equal(app.runs.length, 3);
  assert.equal(app.$("results").children[0].dataset.resultId, other.resultId);
  assert.equal(app.$("results").children[0].querySelector("img").src, dataUrl);
  assert.equal(
    app.calls.filter(
      (operation) =>
        operation.action === "fetch-image" &&
        operation.url === other.previewUrl,
    ).length,
    1,
  );
});

test("revocation rejects a late provider result and permission restoration can load again", async () => {
  const load = deferred();
  const app = await mount({
    run: () => load.promise,
  });
  app.update({ grantedPermissions: [] });
  load.resolve(wallpaperView());
  await settle();
  assert.equal(app.$("results").querySelectorAll(".result").length, 0);
  assert.equal(app.$("permission").hidden, false);
  assert.equal(app.$("search").disabled, true);
  app.update({ grantedPermissions: permissions });
  await settle();
  assert.equal(app.$("search").disabled, false);
  assert.equal(app.calls.filter((call) => call.action === "list").length, 2);
});

test("malformed service responses preserve prior gallery and reject unsafe previews", async () => {
  let fail = false;
  const app = await mount({
    request: (operation) => {
      if (operation.action === "list" && fail)
        return { revision: 2, limit: 24, items: [null] };
      if (operation.action === "preview")
        return { dataUrl: "https://example.com/tracker.jpg" };
    },
  });
  fail = true;
  app.$("refresh").click();
  await settle();
  assert.equal(app.$("gallery").children[0].dataset.wallpaperId, item.id);
  app.$("search").click();
  await settle();
  app.$("results").children[0].click();
  await settle();
  assert.equal(app.$("preview-image"), null);
  assert.ok(app.notifications.includes("Operation failed. Please retry."));
  assert.ok(
    app.calls.some(
      (call) =>
        call.service === "resources" &&
        call.action === "release" &&
        call.resourceId === resource.resourceId,
    ),
  );
});

test("gallery follows the host capacity and preserves results when capacity metadata is invalid", async () => {
  const expanded = {
    ...library,
    limit: 48,
    items: Array.from({ length: 25 }, (_, index) => ({
      ...item,
      id: index.toString(16).padStart(64, "0"),
    })),
  };
  let response = expanded;
  const app = await mount({
    request: (operation) =>
      operation.action === "list" ? structuredClone(response) : undefined,
  });
  assert.equal(app.$("gallery").children.length, 25);
  assert.equal(app.$("local-summary").textContent, "25 / 48 images");

  for (const invalid of [
    { ...expanded, limit: "48" },
    { ...expanded, items: [], limit: 0 },
    { ...expanded, limit: 24 },
  ]) {
    response = invalid;
    app.$("refresh").click();
    await settle();
    assert.equal(app.$("gallery").children.length, 25);
    assert.equal(app.$("local-summary").textContent, "25 / 48 images");
  }
  assert.ok(app.notifications.includes("Operation failed. Please retry."));
});

test("rename dialog is named, localizes without losing the draft and validates Enter", async () => {
  const app = await mount({ saved: { tab: "local" } });
  const rename = galleryAction(app, "rename");
  rename.focus();
  rename.click();
  await settle();
  assert.equal(
    app.$("confirm-dialog").getAttribute("aria-labelledby"),
    "dialog-title",
  );
  app.$("rename-value").value = "New mountain";
  app.update({ locale: "zh-CN" });
  assert.equal(app.$("dialog-title").textContent, "重命名");
  assert.equal(app.$("rename-value").value, "New mountain");
  app.$("rename-value").value = "bad\u0000name";
  app.$("dialog-confirm").click();
  assert.equal(app.$("confirm-dialog").open, true);
  assert.ok(app.$("rename-value").validationMessage);
  app.$("rename-value").value = "New mountain";
  app.$("rename-value").oninput();
  app.$("rename-value").onkeydown({ key: "Enter", preventDefault() {} });
  await settle();
  assert.equal(app.calls.at(-1).action, "rename");
  assert.equal(app.calls.at(-1).name, "New mountain");
  assert.equal(app.document.activeElement.dataset.action, "rename");
  assert.equal(
    app.document.activeElement.closest(".card").dataset.wallpaperId,
    item.id,
  );
});

test("apply and confirmed removal retain the gallery on a settings revision conflict", async () => {
  const app = await mount({
    saved: { tab: "local" },
    request: (operation) =>
      ["apply", "remove"].includes(operation.action)
        ? Promise.reject({ code: "SETTINGS_CONFLICT" })
        : undefined,
  });
  const action = (name) => galleryAction(app, name);
  action("apply").click();
  await settle();
  const applied = app.calls.find((call) => call.action === "apply");
  assert.equal(applied.expectedRevision, library.revision);
  assert.equal(applied.id, item.id);
  assert.equal(app.notifications.includes("App background updated."), false);
  assert.equal(app.$("gallery").children[0].className, "card");
  action("remove").click();
  await settle();
  app.update({ locale: "zh-CN" });
  assert.equal(app.$("dialog-title").textContent, "移出图库？");
  app.$("dialog-confirm").click();
  await settle();
  const removed = app.calls.find((call) => call.action === "remove");
  assert.equal(removed.expectedRevision, library.revision);
  assert.equal(removed.id, item.id);
  assert.equal(app.$("gallery").children[0].dataset.wallpaperId, item.id);
  assert.ok(
    app.notifications.some((message) =>
      message.includes("设置已在其他窗口改变"),
    ),
  );
});

test("gallery images apply the background and named action icons stay beside the image name", async () => {
  const app = await mount({ saved: { tab: "local" } });
  const apply = galleryAction(app, "apply");
  assert.equal(apply.querySelector("img").src, dataUrl);
  assert.equal(apply.querySelector("img").alt, "");
  assert.equal(apply.getAttribute("aria-pressed"), "false");
  for (const [action, label] of [
    ["rename", /rename/i],
    ["remove", /remove/i],
  ]) {
    const icon = galleryAction(app, action);
    assert.match(icon.title, label);
    assert.match(icon.getAttribute("aria-label"), label);
    assert.ok(icon.getAttribute("aria-label").includes(item.name));
    assert.equal(
      icon.closest(".card-heading").querySelector("h2").textContent,
      item.name,
    );
  }
  app.update({ locale: "zh-CN" });
  assert.match(
    galleryAction(app, "rename").getAttribute("aria-label"),
    /重命名/,
  );
  assert.match(
    galleryAction(app, "remove").getAttribute("aria-label"),
    /移除|删除/,
  );
});

test("direct card actions keep the selected wallpaper identity and return focus to the same action", async () => {
  const other = { ...item, id: "e".repeat(64), name: "Another wallpaper" };
  const response = { revision: 7, limit: 24, items: [item, other] };
  const app = await mount({
    saved: { tab: "local" },
    request: (operation) =>
      operation.action === "list"
        ? structuredClone(response)
        : operation.action === "apply"
          ? {
              ...response,
              items: response.items.map((entry) => ({
                ...entry,
                active: entry.id === operation.id,
              })),
            }
          : undefined,
  });
  const apply = galleryAction(app, "apply", other.id);
  apply.focus();
  apply.click();
  await settle();
  const applied = app.calls.find((operation) => operation.action === "apply");
  assert.equal(applied.id, other.id);
  assert.equal(applied.expectedRevision, 7);
  assert.equal(
    galleryAction(app, "apply", other.id).getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.document.activeElement.dataset.action, "apply");
  assert.equal(
    app.document.activeElement.closest(".card").dataset.wallpaperId,
    other.id,
  );
  for (const name of ["rename", "remove"]) {
    const action = galleryAction(app, name, other.id);
    action.focus();
    action.click();
    await settle();
    if (name === "rename")
      assert.equal(app.$("rename-value").value, other.name);
    app.$("dialog-cancel").click();
    await settle();
    assert.equal(app.document.activeElement.dataset.action, name);
    assert.equal(
      app.document.activeElement.closest(".card").dataset.wallpaperId,
      other.id,
    );
  }
  assert.equal(
    app.calls.some((operation) =>
      ["rename", "remove"].includes(operation.action),
    ),
    false,
  );
  assert.equal(
    app.document.activeElement.closest(".card").dataset.wallpaperId,
    other.id,
  );
});

test("confirming removal returns focus to import after the focused card disappears", async () => {
  const app = await mount({
    saved: { tab: "local" },
    request: (operation) =>
      operation.action === "remove"
        ? { ...library, revision: 2, items: [] }
        : undefined,
  });
  const remove = galleryAction(app, "remove");
  remove.focus();
  remove.click();
  await settle();
  assert.equal(app.$("confirm-dialog").open, true);
  app.$("dialog-confirm").click();
  await settle();
  assert.equal(app.$("gallery").querySelectorAll(".card").length, 0);
  assert.equal(app.document.activeElement, app.$("import"));
  assert.equal(app.$("dialog-cancel").getClientRects().length, 0);
  const removed = app.calls.find((operation) => operation.action === "remove");
  assert.equal(removed.id, item.id);
  assert.equal(removed.expectedRevision, library.revision);
});

test("hiding or revoking access prevents stale card actions", async () => {
  const app = await mount({ saved: { tab: "local" } });
  const staleApply = galleryAction(app, "apply");
  app.update({ active: false });
  staleApply.click();
  await settle();
  app.update({ active: true });
  const revokedApply = galleryAction(app, "apply");
  app.update({ grantedPermissions: [] });
  revokedApply.click();
  await settle();
  assert.equal(
    app.calls.some((operation) => operation.action === "apply"),
    false,
  );
});

test("changing a provider loads its collection and a failed refresh retains visible results", async () => {
  let fail = false;
  const app = await mount({
    run: (action, fields) => {
      if (fail) return Promise.reject({ code: "EXTENSION_TIMEOUT" });
      return wallpaperView(fields.provider);
    },
  });
  app.$("provider").value = "commons";
  app.$("provider").onchange();
  await settle();
  assert.deepEqual(app.runs.at(-1), {
    action: "load",
    fields: { provider: "commons", query: "", category: "all" },
  });
  fail = true;
  app.$("search").click();
  await settle();
  assert.equal(app.$("results").children.length, 1);
  assert.equal(app.$("provider").value, "commons");
  assert.ok(app.notifications.some((message) => /timed out/i.test(message)));
});

test("local gallery works without optional network grants and image preview cannot run", async () => {
  const app = await mount({ grants: ["wallpapers:manage"] });
  assert.equal(app.$("search").disabled, true);
  assert.equal(app.$("refresh").disabled, false);
  assert.equal(app.$("import").disabled, false);
  assert.equal(app.$("network-mode"), null);
  assert.equal(app.calls.filter((call) => call.action === "list").length, 1);
  assert.equal(
    app.calls.some((call) => call.service === "resources"),
    false,
  );
  assert.equal(app.runs.length, 0);
});

for (const links of [true, false])
  test(`result metadata remains selectable without image access and source links ${links ? "use their independent grant" : "require their independent grant"}`, async () => {
    const app = await mount({
      grants: [
        "wallpapers:manage",
        "network:https",
        ...(links ? ["links:open"] : []),
      ],
    });
    const result = app.$("results").children[0];
    assert.equal(result.disabled, false);
    result.click();
    await settle();
    assert.equal(app.$("preview-image"), null);
    assert.match(
      app.$("preview-stage").querySelector(".placeholder").textContent,
      /Source details remain available/,
    );
    app.$("preview-info").focus();
    app.$("preview-info").click();
    assert.equal(app.$("image-info-dialog").open, true);
    assert.ok(
      app
        .$("image-info-body")
        .children.some((child) =>
          child.textContent?.includes(remote.sourceUrl),
        ),
    );
    app.$("image-info-close").click();
    assert.equal(app.$("download-original").disabled, true);
    assert.equal(app.$("add-to-gallery").disabled, true);
    assert.equal(app.$("open-source").disabled, !links);
    app.update({ locale: "zh-CN" });
    assert.equal(app.$("open-source").disabled, !links);
    app.$("open-source").click();
    await settle();
    assert.equal(
      app.calls.some((call) => call.service === "resources"),
      false,
    );
    const opened = app.calls.filter((call) => call.service === "links");
    assert.equal(opened.length, links ? 1 : 0);
    if (links) assert.equal(opened[0].url, remote.sourceUrl);
    assert.equal(app.$("refresh").disabled, false);
  });

test("image cards load before selection and a failed card can retry in its preview dialog", async () => {
  let previews = 0;
  const other = { ...remote, resultId: "c".repeat(16), title: "Another image" };
  const third = { ...remote, resultId: "d".repeat(16), title: "Third image" };
  const app = await mount({
    run: () => wallpaperView("upx8", [remote, other, third]),
    request: (operation, service) =>
      service === "resources" &&
      operation.action === "preview" &&
      previews++ === 0
        ? Promise.reject({ code: "EXTENSION_TIMEOUT" })
        : undefined,
  });
  assert.equal(app.$("results").children[1].querySelector("img").src, dataUrl);
  assert.equal(app.$("results").children[2].querySelector("img").src, dataUrl);
  app.$("results").children[0].click();
  await settle();
  assert.equal(app.$("preview-dialog").open, true);
  assert.equal(app.$("preview-image"), null);
  app.$("retry-preview").click();
  await settle();
  assert.equal(app.$("preview-image").alt, remote.title);
  assert.equal(app.$("preview-image").src, dataUrl);
  assert.equal(
    app.$("results").children[0].getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.calls.filter((call) => call.action === "release").length, 4);
  app.$("preview-close").click();
  assert.equal(app.$("preview-dialog").open, false);
});

test("typed image errors explain the failure and batch retry reloads only the failed cards", async () => {
  const other = {
    ...remote,
    resultId: "c".repeat(16),
    previewUrl: "https://example.com/healthy.jpg",
  };
  const third = {
    ...remote,
    resultId: "d".repeat(16),
    previewUrl: "https://example.com/another-failure.jpg",
  };
  let fail = true;
  const app = await mount({
    run: () => wallpaperView("upx8", [remote, other, third]),
    request: (operation, service) =>
      service === "resources" &&
      operation.action === "fetch-image" &&
      operation.url !== other.previewUrl &&
      fail
        ? Promise.reject({
            code: "EXTENSION_IMAGE_REDIRECT",
            message: "untrusted server detail",
          })
        : undefined,
  });
  assert.equal(app.$("retry-failed").hidden, false);
  assert.match(app.$("retry-failed").textContent, /2/);
  const state = app.$("results").children[0].querySelector(".image-state");
  assert.match(state.querySelector("span").textContent, /image address moved/i);
  assert.equal(app.$("results").children[1].querySelector("img").src, dataUrl);
  app.$("results").children[0].click();
  assert.match(
    app.$("preview").querySelector(".placeholder").textContent,
    /image address redirects/i,
  );
  app.$("preview-close").click();
  fail = false;
  app.$("retry-failed").click();
  await settle();
  assert.equal(app.$("results").querySelectorAll("img").length, 3);
  assert.equal(app.$("retry-failed").hidden, true);
  app.$("results").children[0].click();
  assert.equal(app.$("preview-image").src, dataUrl);
  const downloads = app.calls.filter(
    (operation) => operation.action === "fetch-image",
  );
  assert.equal(
    downloads.filter((operation) => operation.url === remote.previewUrl).length,
    2,
  );
  assert.equal(
    downloads.filter((operation) => operation.url === other.previewUrl).length,
    1,
  );
  assert.equal(
    downloads.filter((operation) => operation.url === third.previewUrl).length,
    2,
  );
});

test("an entirely failed batch reports its typed reason once across environment renders", async () => {
  const other = { ...remote, resultId: "c".repeat(16) };
  const app = await mount({
    run: () => wallpaperView("upx8", [remote, other]),
    request: (operation, service) =>
      service === "resources" && operation.action === "fetch-image"
        ? Promise.reject({ code: "EXTENSION_NETWORK_BLOCKED" })
        : undefined,
  });
  assert.equal(app.notifications.length, 1);
  assert.match(app.notifications[0], /allowed public address/i);
  app.update({ locale: "zh-CN" });
  app.update({ locale: "en" });
  await settle();
  assert.equal(app.notifications.length, 1);
  assert.equal(app.$("retry-failed").hidden, false);
  assert.match(app.$("retry-failed").textContent, /2/);
});

for (const restore of [false, true])
  test(`revoking optional image permission discards a late preview${restore ? " after regrant" : ""} and releases its resource`, async () => {
    const preview = deferred();
    let requests = 0;
    const app = await mount({
      request: (operation, service) =>
        service === "resources" && operation.action === "preview"
          ? requests++ === 0
            ? preview.promise
            : new Promise(() => {})
          : undefined,
    });
    app.$("results").children[0].click();
    await settle();
    assert.ok(
      app.calls.some(
        (call) => call.service === "resources" && call.action === "preview",
      ),
    );
    app.update({
      grantedPermissions: permissions.filter(
        (permission) => permission !== "network:images",
      ),
    });
    if (restore) app.update({ grantedPermissions: permissions });
    preview.resolve({ dataUrl });
    await settle();
    assert.equal(app.$("preview-image"), null);
    assert.equal(app.$("results").querySelectorAll("img").length, 0);
    assert.equal(app.$("refresh").disabled, false);
    assert.ok(
      app.calls.some(
        (call) =>
          call.service === "resources" &&
          call.action === "release" &&
          call.resourceId === resource.resourceId,
      ),
    );
  });

test("changing an unrelated grant does not strand an in-flight thumbnail", async () => {
  const preview = deferred();
  let previews = 0;
  const app = await mount({
    request: (operation, service) =>
      service === "resources" &&
      operation.action === "preview" &&
      previews++ === 0
        ? preview.promise
        : undefined,
  });
  app.update({
    grantedPermissions: permissions.filter(
      (permission) => permission !== "links:open",
    ),
  });
  preview.resolve({ dataUrl });
  await settle();
  assert.equal(app.$("results").querySelector("img")?.src, dataUrl);
});

test("all 24 image cards receive thumbnails automatically with at most two downloads in flight", async () => {
  const entries = Array.from({ length: 24 }, (_, index) => ({
    ...remote,
    resultId: index.toString(16).padStart(16, "0"),
    title: `Wallpaper ${index + 1}`,
    previewUrl: `https://example.com/preview-${index}.jpg`,
  }));
  const pending = [];
  let inFlight = 0,
    maximum = 0;
  const app = await mount({
    run: () => wallpaperView("upx8", entries),
    request(operation) {
      if (operation.action !== "fetch-image") return;
      const receipt = deferred();
      const index = pending.length;
      pending.push(receipt);
      maximum = Math.max(maximum, ++inFlight);
      return receipt.promise.then(() => {
        inFlight--;
        return { ...resource, resourceId: `image-${index}` };
      });
    },
  });
  assert.equal(app.$("results").children.length, 24);
  assert.equal(pending.length, 2);
  for (let index = 0; index < 24; index++) {
    assert.ok(pending[index], `thumbnail ${index + 1} is scheduled`);
    pending[index].resolve();
    await settle();
  }
  assert.equal(maximum, 2);
  const resources = app.calls.filter(
    (operation) => operation.service === "resources",
  );
  for (const action of ["fetch-image", "preview", "release"])
    assert.equal(
      resources.filter((operation) => operation.action === action).length,
      24,
    );
  assert.equal(
    resources.length,
    72,
    "all 24 thumbnail lifecycles complete without a start timer",
  );
  for (const card of app.$("results").children) {
    const image = card.querySelector("img");
    assert.ok(image, "the card shows an image before the user selects it");
    assert.equal(image.src, dataUrl);
    assert.equal(card.getAttribute("aria-pressed"), "false");
  }
  assert.equal(app.$("preview-dialog").open, false);
});

for (const hiddenBy of ["host activity", "document visibility"])
  test(`a page hidden by ${hiddenBy} stops scheduling thumbnails and resumes its queue`, async () => {
    const entries = Array.from({ length: 5 }, (_, index) => ({
      ...remote,
      resultId: index.toString(16).padStart(16, "0"),
      previewUrl: `https://example.com/preview-${index}.jpg`,
    }));
    const pending = [];
    const app = await mount({
      run: () => wallpaperView("upx8", entries),
      request(operation) {
        if (operation.action !== "fetch-image") return;
        const receipt = deferred();
        pending.push(receipt);
        return receipt.promise.then(() => resource);
      },
    });
    assert.equal(pending.length, 2);
    if (hiddenBy === "host activity") app.update({ active: false });
    else app.setVisibility("hidden");
    assert.equal(app.document.documentElement.dataset.galleryVisible, "false");
    pending[0].resolve();
    pending[1].resolve();
    await settle();
    assert.equal(
      pending.length,
      2,
      "hidden surfaces start no additional downloads",
    );
    if (hiddenBy === "host activity") app.update({ active: true });
    else app.setVisibility("visible");
    await settle();
    assert.ok(pending.length > 2);
    for (let index = 2; index < pending.length; index++) {
      pending[index].resolve();
      await settle();
    }
    assert.equal(app.$("results").querySelectorAll("img").length, 5);
  });

for (const hiddenBy of ["host activity", "document visibility"])
  test(`an initially hidden page waits for ${hiddenBy} before loading its first collection`, async () => {
    const app = await mount({
      active: hiddenBy !== "host activity",
      visibility: hiddenBy === "document visibility" ? "hidden" : "visible",
    });
    assert.equal(app.calls.length, 0);
    assert.equal(app.runs.length, 0);
    if (hiddenBy === "host activity") app.update({ active: true });
    else app.setVisibility("visible");
    await settle();
    assert.equal(
      app.calls.filter((operation) => operation.action === "list").length,
      1,
    );
    assert.deepEqual(app.runs, [
      {
        action: "load",
        fields: { provider: "upx8", query: "", category: "all" },
      },
    ]);
    assert.equal(app.$("results").querySelectorAll("img").length, 1);
    app.update({ locale: "zh-CN" });
    await settle();
    assert.equal(
      app.runs.length,
      1,
      "environment renders do not reload a completed collection",
    );
  });

test("an initially busy host defers requests until the first available snapshot", async () => {
  const app = await mount({ hostBusy: true });
  assert.equal(app.calls.length, 0);
  assert.equal(app.runs.length, 0);
  app.update({ busy: false });
  await settle();
  assert.equal(
    app.calls.filter((operation) => operation.action === "list").length,
    1,
  );
  assert.deepEqual(app.runs, [
    {
      action: "load",
      fields: { provider: "upx8", query: "", category: "all" },
    },
  ]);
  assert.equal(app.$("results").querySelector("img")?.src, dataUrl);
  app.update({ locale: "zh-CN", busy: false });
  await settle();
  assert.equal(app.runs.length, 1);
});

test("invalid or obsolete result state cannot replace the currently validated collection", async () => {
  const app = await mount();
  const validState = JSON.parse(wallpaperView().state);
  for (const state of [
    { ...validState, schema: 2 },
    { ...validState, items: [null] },
    { ...validState, items: [remote, remote] },
    {
      ...validState,
      items: [{ ...remote, originalUrl: "file:///private.jpg" }],
    },
  ]) {
    app.update({ view: { state: JSON.stringify(state) } });
    assert.equal(app.$("results").children.length, 1);
    assert.equal(
      app.$("results").children[0].dataset.resultId,
      remote.resultId,
    );
  }
});

test("audit: a failed checkpoint remains pending for the next environment update", async () => {
  let failNext = false;
  const app = await mount({
    checkpoint() {
      if (failNext) {
        failNext = false;
        return Promise.reject(new Error("transient failure"));
      }
      return Promise.resolve();
    },
  });
  failNext = true;
  app.$("query").value = "retained ocean draft";
  app.$("query").oninput();
  await app.flushTimers();
  const count = app.checkpoints.length;
  app.update({ locale: "zh-CN" });
  await settle();
  assert.equal(app.checkpoints.length, count + 1);
  assert.equal(app.checkpoints.at(-1).value.query, "retained ocean draft");
});
test("audit: selected metadata follows the current record with the same image identity", async () => {
  const app = await mount();
  app.$("results").children[0].click();
  app.update({
    view: wallpaperView("upx8", [
      { ...remote, title: "Updated mountain", width: 1920, height: 1080 },
    ]),
  });
  await settle();
  assert.equal(app.$("preview-title").textContent, "Updated mountain");
});
test("audit: native service actions are disabled during a busy host snapshot", async () => {
  const app = await mount();
  app.update({ busy: true });
  assert.equal(app.$("import").disabled, true);
  assert.equal(galleryAction(app, "rename").disabled, true);
});
test("audit: a numeric image identity cannot replace a validated collection", async () => {
  const app = await mount();
  app.update({
    view: wallpaperView("upx8", [{ ...remote, resultId: 1234567890123456 }]),
  });
  assert.equal(app.$("results").children[0].dataset.resultId, remote.resultId);
});

test("a newer checkpoint draft takes precedence over a failed in-flight draft", async () => {
  const failed = deferred();
  const app = await mount({
    checkpoint(value) {
      return value.query === "old draft" ? failed.promise : Promise.resolve();
    },
  });
  app.$("query").value = "old draft";
  app.$("query").oninput();
  await app.flushTimers();
  app.$("query").value = "new draft";
  app.$("query").oninput();
  failed.reject(new Error("transient checkpoint failure"));
  await settle();
  app.update({ locale: "zh-CN" });
  await settle();
  assert.equal(app.checkpoints.at(-1).value.query, "new draft");
  assert.equal(
    app.checkpoints.filter((entry) => entry.value.query === "old draft").length,
    1,
  );
});
test("empty and control-character image titles cannot replace a valid collection", async () => {
  const app = await mount();
  for (const title of ["", "   ", "bad\u0000title"]) {
    app.update({ view: wallpaperView("upx8", [{ ...remote, title }]) });
    assert.equal(
      app.$("results").children[0].querySelector("strong").textContent,
      remote.title,
    );
  }
});

test("a terminal missing-resource release is not repeated by later environment snapshots", async () => {
  const app = await mount({
    request: (operation, service) =>
      service === "resources" && operation.action === "release"
        ? Promise.reject({ code: "NOT_FOUND", retryable: false })
        : undefined,
  });
  const releases = () =>
    app.calls.filter((operation) => operation.action === "release");
  assert.equal(releases().length, 1);
  for (const environment of [
    { locale: "zh-CN" },
    { active: false },
    { active: true },
    { locale: "en" },
  ]) {
    app.update(environment);
    await settle();
  }
  assert.equal(releases().length, 1);
  assert.equal(app.$("results").querySelector("img").src, dataUrl);
});

test("a temporarily busy resource release remains pending until a visible retry succeeds", async () => {
  let available = false;
  const app = await mount({
    request: (operation, service) =>
      service === "resources" && operation.action === "release"
        ? available
          ? null
          : Promise.reject({ code: "EXTENSION_BUSY", retryable: true })
        : undefined,
  });
  const releases = () =>
    app.calls.filter((operation) => operation.action === "release");
  const beforeReturn = releases().length;
  assert.ok(beforeReturn >= 1);
  app.update({ active: false });
  available = true;
  await settle();
  assert.equal(releases().length, beforeReturn);
  app.update({ active: true });
  await settle();
  assert.equal(releases().length, beforeReturn + 1);
  app.update({ locale: "zh-CN" });
  await settle();
  assert.equal(releases().length, beforeReturn + 1);
});
