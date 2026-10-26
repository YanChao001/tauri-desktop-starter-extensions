import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

// Execute the real author script against a small DOM and an explicit SDK boundary
// substitute. Native TCP, layout and the real SDK are verified by the host suite.
const html = readFileSync(new URL("./main.html", import.meta.url), "utf8");
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const peerId = "b".repeat(32);
const flush = () => new Promise((resolve) => setImmediate(resolve));
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function status() {
  return {
    running: true,
    nickname: "My device",
    nodeId: "a".repeat(32),
    port: 45873,
    addresses: ["192.168.1.20"],
    lastError: null,
    peers: [
      {
        id: peerId,
        nickname: "Friend",
        address: "192.168.1.21",
        port: 45873,
        unread: 0,
        lastSeen: 1761400000000,
        connected: true,
      },
    ],
    messages: [
      {
        id: "c".repeat(32),
        peerId,
        sender: "a".repeat(32),
        nickname: "My device",
        text: "Literal <b>text</b>",
        at: 1761400000000,
        outgoing: true,
        delivery: "sent",
      },
    ],
  };
}
async function setup({
  service = () => Promise.resolve(status()),
  checkpoint = () => Promise.resolve(),
  run = () => Promise.resolve(),
  saved = null,
} = {}) {
  const nodes = new Map(),
    events = new Map(),
    timers = new Map();
  let timerId = 0,
    change;
  const notices = [],
    calls = [],
    checkpoints = [],
    runs = [],
    saves = [];
  let snapshot = {
    locale: "en",
    grantedPermissions: ["messaging:lan"],
    fields: {},
    checkpoint: saved,
    busy: false,
  };
  class Element {
    constructor(tagName, text = "") {
      this.tagName = tagName;
      this.childNodes = [];
      this.attributes = new Map();
      this.dataset = {};
      this.value = "";
      this.disabled = false;
      this.hidden = false;
      this.classList = { toggle() {} };
      this.scrollHeight = 100;
      this.scrollTop = 0;
      this.clientHeight = 100;
      if (text) this.textContent = text;
    }
    get textContent() {
      return this.childNodes.map((node) => node.textContent).join("");
    }
    set textContent(value) {
      this.childNodes = [{ textContent: String(value), parentElement: this }];
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) ?? null;
    }
    append(...children) {
      for (const child of children) {
        child.parentElement = this;
        this.childNodes.push(child);
      }
    }
    replaceChildren(...children) {
      this.childNodes = [];
      this.append(...children);
    }
    focus() {
      if (!this.disabled) document.activeElement = this;
    }
    closest() {
      return null;
    }
    querySelectorAll(selector) {
      return this.childNodes.flatMap((child) =>
        child instanceof Element
          ? [
              ...(selector === "button" && child.tagName === "button"
                ? [child]
                : []),
              ...child.querySelectorAll(selector),
            ]
          : [],
      );
    }
  }
  for (const match of html
    .slice(0, html.indexOf("<script>"))
    .matchAll(/<(\w+)\b[^>]*\bid="([^"]+)"[^>]*>/g)) {
    const node = new Element(match[1]);
    for (const attr of match[0].matchAll(/([\w-]+)="([^"]*)"/g))
      node.setAttribute(attr[1], attr[2]);
    node.value = node.getAttribute("value") ?? "";
    nodes.set(match[2], node);
  }
  const fixedText = [];
  for (const [id, text] of [
    ["refresh", "刷新"],
    ["confirm-action", "确认操作"],
    ["cancel-action", "取消"],
    ["send", "发送消息"],
  ]) {
    nodes.get(id).textContent = text;
    fixedText.push(nodes.get(id).childNodes[0]);
  }
  const document = {
    body: new Element("body"),
    activeElement: null,
    hidden: false,
    title: "",
    getElementById: (id) => nodes.get(id),
    createElement: (tag) => new Element(tag),
    createTreeWalker: () => {
      let index = -1;
      return {
        nextNode() {
          return ++index < fixedText.length;
        },
        get currentNode() {
          return fixedText[index];
        },
      };
    },
    querySelectorAll: () =>
      [...nodes.values()].filter((node) =>
        ["aria-label", "placeholder", "title"].some((attr) =>
          node.getAttribute(attr),
        ),
      ),
    addEventListener: (name, callback) => events.set(name, callback),
  };
  const starter = {
    ready: Promise.resolve(snapshot),
    onChange(callback) {
      change = callback;
      return () => {};
    },
    notify: (message, options) => {
      notices.push({ message, options });
      return Promise.resolve();
    },
    services: {
      request: async (_name, request) => {
        calls.push(request);
        return service(request, calls.length);
      },
    },
    checkpoint: async (value, dirty) => {
      checkpoints.push({ value, dirty });
      return checkpoint(value, dirty);
    },
    run: async (action, fields) => {
      runs.push({ action, fields });
      return run(action, fields);
    },
    saveArtifact: async (index) => {
      saves.push(index);
      return true;
    },
  };
  vm.runInNewContext(script, {
    document,
    starter,
    TextEncoder,
    NodeFilter: { SHOW_TEXT: 4 },
    IntersectionObserver: class {
      observe() {}
      disconnect() {}
    },
    window: {
      addEventListener: (name, callback) => events.set(name, callback),
    },
    setTimeout: (callback, delay) => {
      timers.set(++timerId, { callback, delay });
      return timerId;
    },
    clearTimeout: (id) => timers.delete(id),
  });
  await flush();
  return {
    nodes,
    notices,
    calls,
    checkpoints,
    runs,
    saves,
    document,
    emit(patch) {
      snapshot = { ...snapshot, ...patch };
      change(snapshot);
    },
    input(id, value) {
      const node = nodes.get(id);
      node.value = value;
      node.oninput?.();
    },
    async tick(delay) {
      for (const [id, timer] of [...timers])
        if (timer.delay === delay) {
          timers.delete(id);
          timer.callback();
        }
      await flush();
    },
    event(name, event = {}) {
      events.get(name)?.(event);
    },
  };
}

test("locale changes translate fixed text, receipts and confirmations without replacing inputs", async () => {
  const env = await setup();
  const { nodes, document } = env;
  assert.equal(nodes.get("refresh").textContent.trim(), "Refresh");
  assert.match(nodes.get("messages").textContent, /Written to connection/);
  assert.match(nodes.get("messages").textContent, /Literal <b>text<\/b>/);
  env.input("draft", "Unsent text");
  nodes.get("draft").focus();
  env.emit({ locale: "zh-CN" });
  assert.equal(document.activeElement, nodes.get("draft"));
  assert.equal(nodes.get("draft").value, "Unsent text");
  assert.match(nodes.get("messages").textContent, /已写入连接/);
  nodes.get("clear").onclick();
  assert.equal(document.activeElement, nodes.get("confirm-action"));
  env.emit({ locale: "en" });
  assert.match(nodes.get("confirmation-text").textContent, /Clear all local/);
  env.event("keydown", { key: "Escape", preventDefault() {} });
  assert.equal(nodes.get("confirmation").hidden, true);
  assert.equal(document.activeElement, nodes.get("clear"));
});

test("offline changes refresh the receipt even when the message list is identical", async () => {
  const state = status();
  const env = await setup({
    service: () => Promise.resolve(structuredClone(state)),
  });
  state.peers[0].connected = false;
  env.nodes.get("refresh").onclick();
  await flush();
  assert.match(
    env.nodes.get("messages").textContent,
    /No delivery receipt \(currently offline\)/,
  );
  assert.equal(env.nodes.get("send").disabled, true);
});

test("background failures after a successful read remain visible and notify once until recovery", async () => {
  let fail = false;
  const env = await setup({
    service: () =>
      fail ? Promise.reject({ code: "IO_ERROR" }) : Promise.resolve(status()),
  });
  fail = true;
  await env.tick(2000);
  assert.equal(env.nodes.get("status").textContent, "Refresh failed");
  assert.match(env.nodes.get("status").title, /out of date/);
  assert.equal(env.notices.length, 1);
  await env.tick(2000);
  assert.equal(env.notices.length, 1);
  fail = false;
  await env.tick(2000);
  assert.match(env.nodes.get("status").textContent, /Listening/);
  fail = true;
  await env.tick(2000);
  assert.equal(env.notices.length, 2);
});

test("a status in flight across revoke and grant cannot restore its stale snapshot", async () => {
  const old = deferred();
  const env = await setup({
    service: (_request, count) =>
      count === 2 ? old.promise : Promise.resolve(status()),
  });
  await env.tick(2000);
  env.emit({ grantedPermissions: [] });
  env.emit({ grantedPermissions: ["messaging:lan"] });
  const stale = status();
  stale.port = 22222;
  old.resolve(stale);
  await flush();
  assert.doesNotMatch(env.nodes.get("status").textContent, /22222/);
  await env.tick(2000);
  assert.match(env.nodes.get("status").textContent, /45873/);
});

test("send preserves an edited draft even if its text returns to the original value", async () => {
  const sent = deferred();
  const env = await setup({
    service: (request) =>
      request.action === "send" ? sent.promise : Promise.resolve(status()),
  });
  env.input("draft", "Same text");
  env.nodes.get("send").onclick();
  assert.equal(
    env.nodes.get("contacts").querySelectorAll("button")[0].disabled,
    true,
  );
  env.input("draft", "Edited text");
  env.input("draft", "Same text");
  sent.resolve(status());
  await flush();
  assert.equal(env.nodes.get("draft").value, "Same text");
  assert.equal(
    env.calls.filter((request) => request.action === "send").length,
    1,
  );
});

test("checkpoint writes are serialized and coalesce to the latest draft", async () => {
  const pending = deferred();
  const env = await setup({ checkpoint: () => pending.promise });
  for (let index = 0; index < 30; index++) env.input("draft", `Draft ${index}`);
  assert.equal(env.checkpoints.length, 1);
  pending.resolve();
  await flush();
  await env.tick(150);
  assert.equal(env.checkpoints.length, 2);
  assert.equal(env.checkpoints[1].value.draft, "Draft 29");
  assert.equal(env.checkpoints[1].dirty, true);
});

test("future and malformed checkpoints preserve originals and refuse business writes", async () => {
  for (const saved of [
    { version: 9, draft: "original" },
    { version: 1, draft: "original", port: "0" },
  ]) {
    const env = await setup({ saved });
    assert.equal(env.nodes.get("draft").disabled, true);
    assert.equal(env.nodes.get("start").disabled, true);
    assert.equal(env.checkpoints.length, 0);
    assert.equal(
      env.calls.filter((request) => request.action !== "status").length,
      0,
    );
    assert.match(env.notices[0].message, /Original data has been preserved/);
  }
});

test("bounded export includes the omission header and ignores an oversized unsent draft", async () => {
  const state = status();
  state.messages = Array.from({ length: 500 }, (_, index) => ({
    ...state.messages[0],
    id: index.toString(16).padStart(32, "0"),
    text: `${index} ${"界".repeat(333)}`,
  }));
  const env = await setup({ service: () => Promise.resolve(state) });
  const draft = "界".repeat(4000);
  env.input("draft", draft);
  assert.equal(env.nodes.get("send").disabled, true);
  await env.nodes.get("export").onclick();
  const fields = env.runs[0].fields;
  assert.equal(fields.draft, "");
  assert.match(fields.export, /^LAN Messenger · Friend\nExported:/);
  assert.match(fields.export, /oldest messages omitted/);
  assert.match(fields.export, /499 界/);
  assert.ok(Buffer.byteLength(fields.export) <= 200 * 1024);
  assert.equal(env.nodes.get("draft").value, draft);
  assert.deepEqual(env.saves, [0]);
});

test("permission loss while preparing export prevents a later save dialog", async () => {
  const pending = deferred();
  const env = await setup({ run: () => pending.promise });
  const exporting = env.nodes.get("export").onclick();
  env.emit({ grantedPermissions: [] });
  pending.resolve();
  await exporting;
  assert.deepEqual(env.saves, []);
});

test("invalid service snapshots never replace the last valid history", async () => {
  for (const mutate of [
    (state) => (state.peers[0].unread = -1),
    (state) => (state.messages[0].at = Infinity),
    (state) => (state.messages[0].peerId = "d".repeat(32)),
    (state) => (state.messages[0].text = "bad\u001btext"),
    (state) => state.peers.push(state.peers[0]),
  ]) {
    const state = status();
    const env = await setup({
      service: (_request, count) =>
        Promise.resolve(count === 1 ? status() : state),
    });
    mutate(state);
    env.nodes.get("refresh").onclick();
    await flush();
    assert.equal(env.nodes.get("status").textContent, "Refresh failed");
    assert.match(env.nodes.get("messages").textContent, /Literal <b>text<\/b>/);
    assert.equal(
      env.nodes.get("contacts").querySelectorAll("button").length,
      1,
    );
  }
});
