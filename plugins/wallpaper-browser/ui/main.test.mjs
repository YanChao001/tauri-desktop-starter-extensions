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
  previewUrl: "https://tse1.mm.bing.net/th?id=test",
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
function searchView(query = "landscape", items = [remote]) {
  return {
    state: JSON.stringify({
      schema: 1,
      query,
      networkMode: "system",
      items,
      pending: null,
      error: "",
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
  for (let index = 0; index < 30; index++) await Promise.resolve();
}

// This small DOM implements only the APIs used by this document. The assertions
// execute the shipped script, including its actual async handlers and renders.
async function mount({
  request,
  run,
  checkpoint,
  saved = null,
  grants = permissions,
} = {}) {
  const document = { activeElement: null };
  class Element {
    constructor(tagName) {
      this.tagName = tagName.toLowerCase();
      this.children = [];
      this.dataset = {};
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
    }
    getAttribute(key) {
      return this.attributes[key] ?? null;
    }
    addEventListener() {}
    focus() {
      document.activeElement = this;
    }
    getClientRects() {
      return this.hidden || this.parentElement?.getClientRects().length === 0
        ? []
        : [{}];
    }
    click() {
      if (!this.disabled) this.onclick?.({ preventDefault() {} });
    }
    showModal() {
      this.open = true;
    }
    close(value) {
      this.returnValue = value;
      this.open = false;
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
  document.querySelectorAll = (selector) =>
    document.body.querySelectorAll(selector);
  document.querySelector = (selector) => document.body.querySelector(selector);
  document.getElementById = (id) => {
    const find = (element) =>
      element.id === id ? element : element.children.map(find).find(Boolean);
    return find(document.body) ?? null;
  };
  document.addEventListener = () => {};
  for (const match of html.matchAll(
    /<([\w-]+)\b([^>]*\bid="([^"]+)"[^>]*)>/g,
  )) {
    const element = new Element(match[1]);
    element.id = match[3];
    element.value = match[2].match(/\bvalue="([^"]*)"/)?.[1] ?? "";
    element.hidden = /\bhidden\b/.test(match[2]);
    element.className = match[2].match(/\bclass="([^"]*)"/)?.[1] ?? "";
    for (const attribute of match[2].matchAll(/\b(aria-[\w-]+)="([^"]*)"/g))
      element.setAttribute(attribute[1], attribute[2]);
    document.body.append(element);
  }
  const $ = (id) => document.getElementById(id);
  const tabs = new Element("div");
  tabs.setAttribute("role", "tablist");
  document.body.append(tabs);
  const help = new Element("span");
  help.className = "help";
  document.body.children = document.body.children.filter(
    (child) => child !== $("source-help-button"),
  );
  const helpButton = new Element("button");
  helpButton.id = "source-help-button";
  help.append(helpButton);
  document.body.append(help);
  for (const id of ["fit", "network-mode"]) {
    $(id).options = [{ text: "" }, { text: "" }];
    $(id).value = id === "fit" ? "cover" : "system";
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
      const view = await (run?.(action, fields) ?? searchView(fields.query));
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

test("query, connection and fit drafts are checkpointed without searching and restore", async () => {
  const app = await mount();
  app.$("query").value = "draft mountains";
  app.$("query").oninput();
  await app.flushTimers();
  app.$("network-mode").value = "direct";
  app.$("network-mode").onchange();
  app.$("fit").value = "contain";
  app.$("fit").onchange();
  await settle();
  const saved = app.checkpoints.at(-1);
  assert.equal(saved.dirty, false);
  assert.deepEqual(JSON.parse(JSON.stringify(saved.value)), {
    query: "draft mountains",
    tab: "remote",
    networkMode: "direct",
    fit: "contain",
  });
  assert.equal(
    app.runs.some((call) => call.action === "search"),
    false,
  );
  const reopened = await mount({ saved: saved.value });
  assert.equal(reopened.$("query").value, "draft mountains");
  assert.equal(reopened.$("network-mode").value, "direct");
  assert.equal(reopened.$("fit").value, "contain");
});

test("switching to the gallery during a search refreshes after the active operation", async () => {
  const search = deferred();
  const app = await mount({
    run: () => search.promise,
  });
  app.$("search").click();
  app.$("local-tab").click();
  assert.equal(app.calls.filter((call) => call.action === "list").length, 1);
  search.resolve(searchView());
  await settle();
  assert.equal(app.calls.filter((call) => call.action === "list").length, 2);
  assert.equal(app.$("local-panel").hidden, false);
  assert.equal(app.$("refresh").disabled, false);
});

test("environment renders preserve focused results, gallery actions and image details", async () => {
  const app = await mount();
  app.$("search").click();
  await settle();
  let result = app.$("results").children[0];
  result.focus();
  app.update({ locale: "zh-CN" });
  assert.equal(app.document.activeElement.dataset.resultId, remote.resultId);
  result = app.$("results").children[0];
  result.click();
  await settle();
  app.$("preview").querySelector("details").open = true;
  app.$("image-details-summary").focus();
  app.update({ locale: "en" });
  assert.equal(app.$("preview").querySelector("details").open, true);
  assert.equal(app.document.activeElement.id, "image-details-summary");
  app.$("local-tab").click();
  await settle();
  const rename = app
    .$("gallery")
    .querySelectorAll("button[data-action]")
    .find((button) => button.dataset.action === "rename");
  rename.focus();
  app.update({ locale: "zh-CN" });
  assert.equal(app.document.activeElement.dataset.action, "rename");
  assert.equal(
    app.document.activeElement.closest(".card").dataset.wallpaperId,
    item.id,
  );
});

test("revocation rejects a late result and permission restoration can load again", async () => {
  const search = deferred();
  const app = await mount({
    run: () => search.promise,
  });
  app.$("search").click();
  app.update({ grantedPermissions: [] });
  search.resolve(searchView());
  await settle();
  assert.equal(app.$("results").children.length, 0);
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
  assert.ok(app.notifications.includes("The operation failed. Please retry."));
  assert.ok(
    app.calls.some(
      (call) =>
        call.service === "resources" &&
        call.action === "release" &&
        call.resourceId === resource.resourceId,
    ),
  );
});

test("rename dialog is named, localizes without losing the draft and validates Enter", async () => {
  const app = await mount();
  const rename = app
    .$("gallery")
    .querySelectorAll("button[data-action]")
    .find((button) => button.dataset.action === "rename");
  rename.click();
  await settle();
  assert.equal(
    app.$("confirm-dialog").getAttribute("aria-labelledby"),
    "dialog-title",
  );
  app.$("rename-value").value = "New mountain";
  app.update({ locale: "zh-CN" });
  assert.equal(app.$("dialog-title").textContent, "重命名壁纸");
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
});

test("apply and confirmed removal retain the gallery on a settings revision conflict", async () => {
  const app = await mount({
    request: (operation) =>
      ["apply", "remove"].includes(operation.action)
        ? Promise.reject({ code: "SETTINGS_CONFLICT" })
        : undefined,
  });
  const action = (name) =>
    app
      .$("gallery")
      .querySelectorAll("button[data-action]")
      .find((button) => button.dataset.action === name);
  action("apply").click();
  await settle();
  const applied = app.calls.find((call) => call.action === "apply");
  assert.equal(applied.expectedRevision, library.revision);
  assert.equal(applied.id, item.id);
  assert.equal(
    app.notifications.includes("Application background updated."),
    false,
  );
  assert.equal(app.$("gallery").children[0].className, "card");
  action("remove").click();
  await settle();
  app.update({ locale: "zh-CN" });
  assert.equal(app.$("dialog-title").textContent, "移出本机图库？");
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

test("invalid search input sends no Wasm command and failed searches retain query-labelled results", async () => {
  let fail = false;
  const app = await mount({
    run: () => {
      if (fail) return Promise.reject({ code: "EXTENSION_TIMEOUT" });
    },
  });
  app.$("query").value = "bad\u0000query";
  app.$("search").click();
  await settle();
  assert.equal(
    app.runs.some((operation) => operation.action === "search"),
    false,
  );
  app.$("query").value = "mountain";
  app.$("search").click();
  await settle();
  fail = true;
  app.$("query").value = "ocean";
  app.$("search").click();
  await settle();
  assert.match(app.$("result-summary").textContent, /mountain/);
  assert.equal(app.$("results").children.length, 1);
  assert.equal(app.$("query").value, "ocean");
  assert.ok(
    app.notifications.some((message) => message.includes("search timed out")),
  );
});

test("local gallery works without optional network grants and image preview cannot run", async () => {
  const app = await mount({ grants: ["wallpapers:manage"] });
  assert.equal(app.$("search").disabled, true);
  assert.equal(app.$("refresh").disabled, false);
  assert.equal(app.$("import").disabled, false);
  assert.equal(app.$("network-mode").value, "direct");
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
    app.$("search").click();
    await settle();
    const result = app.$("results").children[0];
    assert.equal(result.disabled, false);
    result.click();
    await settle();
    assert.equal(app.$("preview-image"), null);
    assert.ok(
      app
        .$("preview")
        .children.some((child) =>
          child.textContent?.includes("Source details remain available"),
        ),
    );
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

test("a failed new preview retains the previously successful selection and image", async () => {
  let fail = false;
  const other = { ...remote, resultId: "c".repeat(16), title: "Another image" };
  const app = await mount({
    run: () => searchView("landscape", [remote, other]),
    request: (operation, service) =>
      service === "resources" && operation.action === "preview" && fail
        ? Promise.reject({ code: "EXTENSION_TIMEOUT" })
        : undefined,
  });
  app.$("search").click();
  await settle();
  app.$("results").children[0].click();
  await settle();
  assert.equal(app.$("preview-image").alt, remote.title);
  fail = true;
  app.$("results").children[1].click();
  await settle();
  assert.equal(app.$("preview-image").alt, remote.title);
  assert.equal(app.$("preview-image").src, dataUrl);
  assert.equal(
    app.$("results").children[0].getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.calls.filter((call) => call.action === "release").length, 2);
});

for (const restore of [false, true])
  test(`revoking optional image permission discards a late preview${restore ? " after regrant" : ""} and releases its resource`, async () => {
    const preview = deferred();
    const app = await mount({
      request: (operation, service) =>
        service === "resources" && operation.action === "preview"
          ? preview.promise
          : undefined,
    });
    app.$("search").click();
    await settle();
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
