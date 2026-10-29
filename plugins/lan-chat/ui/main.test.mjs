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
    mode: "lan",
    loopbackPeerId: null,
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
        kind: "text",
        attachment: null,
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
  width = 1120,
  height = 720,
  sdkLimit = Infinity,
} = {}) {
  const nodes = new Map(),
    events = new Map(),
    timers = new Map();
  let timerId = 0,
    change;
  const geometry = { width, height };
  const sdk = { pending: 0, maximumPending: 0, rejected: [] };
  function sdkRequest(method, operation) {
    if (sdk.pending >= sdkLimit) {
      sdk.rejected.push(method);
      return Promise.reject({ code: "EXTENSION_BUSY" });
    }
    sdk.pending++;
    sdk.maximumPending = Math.max(sdk.maximumPending, sdk.pending);
    try {
      return Promise.resolve(operation()).finally(() => {
        sdk.pending--;
      });
    } catch (error) {
      sdk.pending--;
      return Promise.reject(error);
    }
  }
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
      const properties = new Map();
      this.style = {
        setProperty: (name, value) => properties.set(name, value),
        getPropertyValue: (name) => properties.get(name) ?? "",
      };
      this.capturedPointers = new Set();
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
      for (const child of this.childNodes) child.parentElement = null;
      this.childNodes = [];
      this.append(...children);
    }
    focus() {
      if (!this.disabled) document.activeElement = this;
    }
    get isConnected() {
      return !!this.parentElement || [...nodes.values()].includes(this);
    }
    contains(target) {
      return (
        this === target ||
        this.childNodes.some(
          (child) => child instanceof Element && child.contains(target),
        )
      );
    }
    setSelectionRange(start, end) {
      this.selectionStart = start;
      this.selectionEnd = end;
    }
    getBoundingClientRect() {
      const id = this.getAttribute("id");
      const narrow = geometry.width <= 600;
      return {
        width: geometry.width,
        height:
          id === "chat-header"
            ? narrow
              ? 62
              : 84
            : id === "chat" && narrow
              ? geometry.height - 130
              : geometry.height,
      };
    }
    setPointerCapture(pointerId) {
      this.capturedPointers.add(pointerId);
    }
    hasPointerCapture(pointerId) {
      return this.capturedPointers.has(pointerId);
    }
    releasePointerCapture(pointerId) {
      this.capturedPointers.delete(pointerId);
      this.onlostpointercapture?.();
    }
    closest() {
      return null;
    }
    querySelectorAll(selector) {
      return this.childNodes.flatMap((child) =>
        child instanceof Element
          ? [
              ...(selector === child.tagName ? [child] : []),
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
    node.hidden = /\bhidden\b/.test(match[0]);
    nodes.set(match[2], node);
  }
  const fixedText = [];
  for (const [id, text] of [
    ["confirm-action", "确认"],
    ["cancel-action", "取消"],
    ["send", "发送"],
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
    createElementNS: (_namespace, tag) => new Element(tag),
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
    notify: (message, options) =>
      sdkRequest("notify", () => {
        notices.push({ message, options });
        return Promise.resolve();
      }),
    services: {
      request: async (_name, request) =>
        sdkRequest("services." + request.action, () => {
          calls.push(request);
          return service(request, calls.length);
        }),
    },
    checkpoint: async (value, dirty) =>
      sdkRequest("checkpoint", () => {
        checkpoints.push({ value, dirty });
        return checkpoint(value, dirty);
      }),
    run: async (action, fields) =>
      sdkRequest("run", () => {
        runs.push({ action, fields });
        return run(action, fields);
      }),
    saveArtifact: async (index) =>
      sdkRequest("saveArtifact", () => {
        saves.push(index);
        return true;
      }),
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
    ResizeObserver: class {
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
    sdk,
    document,
    resize(nextWidth, nextHeight) {
      geometry.width = nextWidth;
      geometry.height = nextHeight;
      events.get("resize")?.();
    },
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
  assert.equal(nodes.get("refresh").getAttribute("aria-label"), "Refresh");
  assert.equal(nodes.get("status").textContent, "LAN online");
  assert.match(nodes.get("messages").textContent, /Awaiting delivery receipt/);
  assert.match(nodes.get("messages").textContent, /Literal <b>text<\/b>/);
  env.input("draft", "Unsent text");
  nodes.get("draft").focus();
  env.emit({ locale: "zh-CN" });
  assert.equal(nodes.get("status").textContent, "局域网在线");
  assert.equal(document.activeElement, nodes.get("draft"));
  assert.equal(nodes.get("draft").value, "Unsent text");
  assert.match(nodes.get("messages").textContent, /等待送达回执/);
  nodes.get("clear").onclick();
  assert.equal(document.activeElement, nodes.get("confirm-action"));
  env.emit({ locale: "en" });
  assert.match(nodes.get("confirmation-text").textContent, /Clear all local/);
  env.event("keydown", { key: "Escape", preventDefault() {} });
  assert.equal(nodes.get("confirmation").hidden, true);
  assert.equal(document.activeElement, nodes.get("settings-toggle"));
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
  assert.equal(env.nodes.get("status").textContent, "LAN online");
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
  assert.doesNotMatch(env.nodes.get("addresses").textContent, /22222/);
  await env.tick(2000);
  assert.match(env.nodes.get("addresses").textContent, /45873/);
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
    { version: 1, draft: "original", port: "not-a-number" },
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
  assert.match(fields.export, /^LAN Chat · Friend\nExported:/);
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

test("connection settings stay collapsed and switching modes requires an explicit start", async () => {
  const state = status();
  const env = await setup({
    service: (request) => {
      if (request.action === "loopback") {
        state.mode = "loopback";
        state.loopbackPeerId = peerId;
      }
      return Promise.resolve(structuredClone(state));
    },
  });
  assert.equal(env.nodes.get("settings-panel").hidden, true);
  env.nodes.get("settings-toggle").onclick();
  assert.equal(env.nodes.get("settings-panel").hidden, false);
  env.nodes.get("mode-loopback").onclick();
  assert.equal(env.nodes.get("lan-settings").hidden, true);
  assert.equal(env.nodes.get("loopback-settings").hidden, false);
  assert.equal(
    env.calls.filter((request) => request.action === "loopback").length,
    0,
  );
  env.nodes.get("loopback").onclick();
  await flush();
  assert.equal(
    env.calls.filter((request) => request.action === "loopback").length,
    1,
  );
  assert.match(env.nodes.get("peer-detail").textContent, /Local loopback/);
  assert.equal(env.nodes.get("status").textContent, "Local loopback");
  env.event("keydown", { key: "Escape", preventDefault() {} });
  assert.equal(env.nodes.get("settings-panel").hidden, true);
  assert.equal(env.document.activeElement, env.nodes.get("settings-toggle"));
});

test("each conversation preserves its own draft and message search finds the right contact", async () => {
  const state = status();
  const other = "d".repeat(32);
  state.peers.push({
    ...state.peers[0],
    id: other,
    nickname: "Another friend",
    lastSeen: 1,
  });
  state.messages.push({
    ...state.messages[0],
    id: "e".repeat(32),
    peerId: other,
    text: "Find this unique phrase",
  });
  const env = await setup({ service: () => Promise.resolve(state) });
  env.input("draft", "First conversation draft");
  env.nodes.get("contacts").querySelectorAll("button")[1].onclick();
  assert.equal(env.nodes.get("draft").value, "");
  env.input("draft", "Second conversation draft");
  env.nodes.get("contacts").querySelectorAll("button")[0].onclick();
  assert.equal(env.nodes.get("draft").value, "First conversation draft");
  await flush();
  await env.tick(150);
  const saved = env.checkpoints.at(-1);
  assert.equal(saved.value.drafts[peerId], "First conversation draft");
  assert.equal(saved.value.drafts[other], "Second conversation draft");
  assert.equal(saved.dirty, true);
  env.input("search", "unique phrase");
  const found = env.nodes.get("contacts").querySelectorAll("button");
  assert.equal(found.length, 1);
  assert.equal(found[0].dataset.peer, other);
});

test("loopback waits for the real handshake before selecting the new helper", async () => {
  const state = status();
  const helper = "d".repeat(32);
  const env = await setup({
    service: (request) => {
      if (request.action === "loopback") {
        state.mode = "loopback";
        state.loopbackPeerId = helper;
        state.peers[0].connected = false;
      }
      return Promise.resolve(structuredClone(state));
    },
  });
  env.input("draft", "Keep the first conversation draft");
  env.nodes.get("loopback").onclick();
  await flush();
  assert.equal(env.nodes.get("peer-name").textContent, "Friend");
  state.peers.push({
    ...state.peers[0],
    id: helper,
    nickname: "Echo helper",
    connected: true,
  });
  await env.tick(2000);
  assert.equal(env.nodes.get("peer-name").textContent, "Echo helper");
  assert.equal(env.nodes.get("draft").value, "");
  env.nodes
    .get("contacts")
    .querySelectorAll("button")
    .find((button) => button.dataset.peer === peerId)
    .onclick();
  assert.equal(
    env.nodes.get("draft").value,
    "Keep the first conversation draft",
  );
});

test("emoji insertion respects the caret and sticker sends leave the text draft intact", async () => {
  const env = await setup();
  env.input("draft", "Hello friend");
  env.nodes.get("draft").setSelectionRange(6, 12);
  env.nodes.get("emoji-toggle").onclick();
  const emoji = env.nodes.get("emoji-grid").querySelectorAll("button")[0];
  emoji.onclick();
  assert.equal(env.nodes.get("draft").value, "Hello 😀");
  assert.equal(env.document.activeElement, env.nodes.get("draft"));
  assert.equal(env.nodes.get("emoji-picker").hidden, true);
  env.nodes.get("emoji-toggle").onclick();
  env.nodes.get("sticker-tab").onclick();
  await env.nodes.get("emoji-grid").querySelectorAll("button")[0].onclick();
  assert.equal(
    env.calls.filter(
      (request) => request.action === "send" && request.kind === "sticker",
    ).length,
    1,
  );
  assert.equal(env.nodes.get("draft").value, "Hello 😀");
});

test("Enter with IME composition and Shift+Enter never send prematurely", async () => {
  const env = await setup();
  env.input("draft", "正在输入");
  let prevented = 0;
  for (const input of [
    { isComposing: true },
    { keyCode: 229 },
    { shiftKey: true },
  ]) {
    env.nodes.get("draft").onkeydown({
      key: "Enter",
      preventDefault() {
        prevented++;
      },
      ...input,
    });
  }
  assert.equal(prevented, 0);
  assert.equal(
    env.calls.filter((request) => request.action === "send").length,
    0,
  );
  env.nodes.get("draft").onkeydown({
    key: "Enter",
    preventDefault() {
      prevented++;
    },
  });
  await flush();
  assert.equal(prevented, 1);
  assert.equal(
    env.calls.filter((request) => request.action === "send").length,
    1,
  );
});

function withImages(count = 1) {
  const state = status();
  state.messages = Array.from({ length: count }, (_, index) => ({
    ...state.messages[0],
    id: (index + 1000).toString(16).padStart(32, "0"),
    kind: "image",
    text: "",
    attachment: {
      name: `Image ${index}.png`,
      mime: "image/png",
      bytes: 80,
      sha256: "f".repeat(64),
    },
  }));
  return state;
}
const thumbnail = "data:image/png;base64,aGVsbG8=";

test("images load bounded inline thumbnails, preview and save through the native boundary", async () => {
  const state = withImages();
  const env = await setup({
    service: (request) =>
      Promise.resolve(
        request.action === "previewAttachment"
          ? { dataUrl: thumbnail }
          : request.action === "saveAttachment"
            ? { cancelled: false, name: "Image.png" }
            : state,
      ),
  });
  assert.equal(
    env.calls.filter((request) => request.action === "previewAttachment")
      .length,
    1,
  );
  const show = env.nodes
    .get("messages")
    .querySelectorAll("button")
    .find((button) => button.dataset.action === "preview");
  show.focus();
  await show.onclick();
  assert.equal(env.nodes.get("attachment-preview").hidden, false);
  assert.equal(env.nodes.get("preview-image").src, thumbnail);
  await env.nodes.get("save-preview").onclick();
  assert.equal(
    env.calls.filter((request) => request.action === "saveAttachment").length,
    1,
  );
  assert.match(env.notices.at(-1).message, /Attachment saved/);
  env.event("keydown", { key: "Escape", preventDefault() {} });
  assert.equal(env.nodes.get("attachment-preview").hidden, true);
  assert.equal(env.document.activeElement.dataset.action, "preview");
  assert.equal(env.nodes.get("preview-image").src, "");
});

test("automatic thumbnails keep only the latest twelve images and share one background read slot", async () => {
  const state = withImages(20);
  const pending = [];
  const env = await setup({
    service: (request) => {
      if (request.action !== "previewAttachment")
        return Promise.resolve(structuredClone(state));
      const result = deferred();
      pending.push(result);
      return result.promise;
    },
  });
  assert.equal(pending.length, 1);
  for (let index = 0; index < 12; index++) {
    pending[index].resolve({ dataUrl: thumbnail });
    await flush();
  }
  assert.equal(pending.length, 12);
  const firstRead = env.calls.find(
    (request) => request.action === "previewAttachment",
  );
  assert.equal(firstRead.messageId, state.messages[8].id);
  env.nodes.get("refresh").onclick();
  await flush();
  assert.equal(pending.length, 12);
  assert.equal(env.nodes.get("messages").querySelectorAll("img").length, 12);
  state.messages = withImages(22).messages;
  env.nodes.get("refresh").onclick();
  await flush();
  assert.equal(pending.length, 13);
  pending[12].resolve({ dataUrl: thumbnail });
  await flush();
  assert.equal(pending.length, 14);
  pending[13].resolve({ dataUrl: thumbnail });
  await flush();
  assert.equal(env.nodes.get("messages").querySelectorAll("img").length, 12);
});

test("a late preview cannot reopen an image after permission revocation", async () => {
  const state = withImages();
  const pending = deferred();
  const env = await setup({
    service: (request) =>
      request.action === "previewAttachment"
        ? pending.promise
        : Promise.resolve(state),
  });
  const show = env.nodes
    .get("messages")
    .querySelectorAll("button")
    .find((button) => button.dataset.action === "preview");
  const opening = show.onclick();
  env.emit({ grantedPermissions: [] });
  pending.resolve({ dataUrl: thumbnail });
  await opening;
  await flush();
  assert.equal(env.nodes.get("attachment-preview").hidden, true);
  assert.equal(env.nodes.get("preview-image").src, "");
});

test("thumbnail completion preserves the focused attachment action", async () => {
  const state = withImages();
  const pending = deferred();
  const env = await setup({
    service: (request) =>
      request.action === "previewAttachment"
        ? pending.promise
        : Promise.resolve(state),
  });
  const save = env.nodes
    .get("messages")
    .querySelectorAll("button")
    .find((button) => button.dataset.action === "save");
  save.focus();
  pending.resolve({ dataUrl: thumbnail });
  await flush();
  assert.equal(env.document.activeElement.dataset.action, "save");
  assert.equal(
    env.document.activeElement.dataset.message,
    state.messages[0].id,
  );
  assert.notEqual(env.document.activeElement, save);
});

test("native attachment picker cancellation preserves the draft without a success notification", async () => {
  const env = await setup({
    service: (request) =>
      Promise.resolve({
        ...status(),
        ...(request.action === "sendAttachment" ? { cancelled: true } : {}),
      }),
  });
  env.input("draft", "Keep while choosing a file");
  await env.nodes.get("send-file").onclick();
  assert.equal(
    env.calls.filter(
      (request) =>
        request.action === "sendAttachment" && request.kind === "file",
    ).length,
    1,
  );
  assert.equal(env.nodes.get("draft").value, "Keep while choosing a file");
  assert.equal(env.notices.length, 0);
});

test("hidden and retained inactive pages stop polling and resume from the last state", async () => {
  const env = await setup();
  const count = env.calls.length;
  env.emit({ active: false });
  await env.tick(2000);
  assert.equal(env.calls.length, count);
  assert.match(env.nodes.get("messages").textContent, /Literal <b>text<\/b>/);
  env.emit({ active: true });
  await flush();
  assert.equal(env.calls.length, count + 1);
  env.document.hidden = true;
  env.event("visibilitychange");
  await env.tick(2000);
  assert.equal(env.calls.length, count + 1);
  env.document.hidden = false;
  env.event("visibilitychange");
  await flush();
  assert.equal(env.calls.length, count + 2);
});

test("restored conversation drafts are validated as a whole before accepting any changes", async () => {
  for (const saved of [
    { version: 1, draft: "Keep original", drafts: { [peerId]: 7 } },
    { version: 1, draft: "Keep original", drafts: { unexpected: "text" } },
    { version: 1, draft: "Keep original", mode: "public" },
  ]) {
    const env = await setup({ saved });
    assert.equal(env.nodes.get("draft").disabled, true);
    assert.equal(env.nodes.get("draft").value, "");
    assert.equal(env.checkpoints.length, 0);
    assert.match(env.notices[0].message, /Original data has been preserved/);
  }
});

test("first native status populates an untouched nickname even when settings already focused it", async () => {
  const pending = deferred();
  const env = await setup({ service: () => pending.promise });
  env.nodes.get("settings-toggle").onclick();
  assert.equal(env.document.activeElement, env.nodes.get("nickname"));
  assert.equal(env.nodes.get("nickname").value, "");
  pending.resolve(status());
  await flush();
  assert.equal(env.nodes.get("nickname").value, "My device");
  assert.equal(env.document.activeElement, env.nodes.get("nickname"));
});

test("first native status preserves a nickname typed while the initial read was in flight", async () => {
  const pending = deferred();
  const env = await setup({ service: () => pending.promise });
  env.nodes.get("settings-toggle").onclick();
  env.input("nickname", "My chosen nickname");
  pending.resolve(status());
  await flush();
  assert.equal(env.nodes.get("nickname").value, "My chosen nickname");
  assert.equal(env.document.activeElement, env.nodes.get("nickname"));
});

function pointer(pointerId, clientX = 0, clientY = 0) {
  return { pointerId, clientX, clientY, button: 0, preventDefault() {} };
}
function key(value, shiftKey = false) {
  return { key: value, shiftKey, preventDefault() {} };
}

test("column resizing captures only its pointer, clamps the preview, and checkpoints on release", async () => {
  const env = await setup();
  const handle = env.nodes.get("conversation-resizer");
  handle.onpointerdown(pointer(7, 300));
  assert.equal(handle.hasPointerCapture(7), true);
  handle.onpointermove(pointer(8, 600));
  assert.equal(handle.getAttribute("aria-valuenow"), "224");
  handle.onpointermove(pointer(7, 344));
  assert.equal(
    env.nodes.get("layout").style.getPropertyValue("--conversation-width"),
    "268px",
  );
  assert.equal(env.checkpoints.length, 0);
  handle.onpointermove(pointer(7, 3000));
  assert.equal(handle.getAttribute("aria-valuenow"), "360");
  handle.onpointerup(pointer(7, 3000));
  assert.equal(handle.hasPointerCapture(7), false);
  assert.equal(env.checkpoints[0].value.layout.conversationWidth, 360);
  assert.equal(env.checkpoints[0].dirty, false);
});

test("splitters support keyboard bounds, larger steps and double-click reset without touching drafts", async () => {
  const env = await setup();
  env.input("draft", "Keep this text while resizing");
  const column = env.nodes.get("conversation-resizer");
  column.onkeydown(key("Home"));
  column.onkeydown(key("ArrowLeft"));
  assert.equal(column.getAttribute("aria-valuenow"), "180");
  column.onkeydown(key("ArrowRight", true));
  assert.equal(column.getAttribute("aria-valuenow"), "228");
  column.onkeydown(key("End"));
  assert.equal(column.getAttribute("aria-valuenow"), "360");
  column.ondblclick();
  assert.equal(column.getAttribute("aria-valuenow"), "224");
  const composer = env.nodes.get("composer-resizer");
  composer.onpointerdown(pointer(3, 0, 400));
  composer.onpointermove(pointer(3, 0, 360));
  composer.onpointerup(pointer(3, 0, 360));
  assert.equal(composer.getAttribute("aria-valuenow"), "208");
  composer.onkeydown(key("ArrowDown"));
  assert.equal(composer.getAttribute("aria-valuenow"), "192");
  composer.onkeydown(key("Home"));
  composer.onkeydown(key("ArrowDown"));
  assert.equal(composer.getAttribute("aria-valuenow"), "128");
  composer.onkeydown(key("End"));
  composer.onkeydown(key("ArrowUp"));
  assert.equal(composer.getAttribute("aria-valuenow"), "360");
  composer.ondblclick();
  assert.equal(composer.getAttribute("aria-valuenow"), "168");
  assert.equal(env.nodes.get("draft").value, "Keep this text while resizing");
  await flush();
  await env.tick(150);
  assert.equal(env.checkpoints.at(-1).value.layout.conversationWidth, 224);
  assert.equal(env.checkpoints.at(-1).value.layout.composerHeight, 168);
  assert.equal(env.checkpoints.at(-1).dirty, true);
});

test("Escape, pointer cancellation and lost capture roll layout previews back without writes", async () => {
  for (const cancel of ["escape", "pointercancel", "lostcapture"]) {
    const env = await setup();
    const handle = env.nodes.get("conversation-resizer");
    handle.onpointerdown(pointer(5, 200));
    handle.onpointermove(pointer(5, 250));
    assert.equal(handle.getAttribute("aria-valuenow"), "274");
    if (cancel === "escape") env.event("keydown", key("Escape"));
    else if (cancel === "pointercancel") handle.onpointercancel(pointer(5));
    else handle.releasePointerCapture(5);
    assert.equal(handle.getAttribute("aria-valuenow"), "224");
    assert.equal(handle.hasPointerCapture(5), false);
    assert.equal(env.checkpoints.length, 0);
  }
});

test("narrow and short viewports clamp effective sizes while preserving restored layout preferences", async () => {
  const env = await setup({
    saved: {
      version: 1,
      layout: { conversationWidth: 320, composerHeight: 340 },
    },
  });
  const column = env.nodes.get("conversation-resizer"),
    composer = env.nodes.get("composer-resizer");
  assert.equal(column.getAttribute("aria-valuenow"), "320");
  assert.equal(composer.getAttribute("aria-valuenow"), "340");
  env.resize(540, 450);
  assert.equal(column.hidden, true);
  assert.equal(column.getAttribute("aria-disabled"), "true");
  assert.equal(column.tabIndex, -1);
  column.onkeydown(key("End"));
  assert.equal(composer.getAttribute("aria-valuenow"), "152");
  env.resize(1120, 720);
  assert.equal(column.hidden, false);
  assert.equal(column.getAttribute("aria-valuenow"), "320");
  assert.equal(composer.getAttribute("aria-valuenow"), "340");
  assert.equal(env.checkpoints.length, 0);
});

test("invalid layout checkpoints preserve their original data and refuse partial restoration", async () => {
  for (const layout of [
    { conversationWidth: 361, composerHeight: 168 },
    { conversationWidth: 224, composerHeight: 95 },
    { conversationWidth: 224, composerHeight: 168.5 },
    { conversationWidth: 224 },
    { conversationWidth: 224, composerHeight: 168, unknown: 1 },
  ]) {
    const env = await setup({
      saved: { version: 1, draft: "Original draft", layout },
    });
    assert.equal(env.nodes.get("draft").disabled, true);
    assert.equal(env.nodes.get("draft").value, "");
    assert.equal(
      env.nodes.get("conversation-resizer").getAttribute("aria-disabled"),
      "true",
    );
    assert.equal(env.checkpoints.length, 0);
    assert.match(env.notices[0].message, /Original data has been preserved/);
  }
});

test("self, contact and loopback avatars use fixed SVG shapes instead of nickname initials", async () => {
  const env = await setup();
  assert.equal(env.nodes.get("my-avatar").textContent, "");
  assert.equal(
    env.nodes.get("my-avatar").querySelectorAll("svg")[0].dataset.icon,
    "self",
  );
  assert.equal(env.nodes.get("peer-avatar").textContent, "");
  assert.equal(
    env.nodes.get("peer-avatar").querySelectorAll("svg")[0].dataset.icon,
    "person",
  );
  const state = status();
  state.mode = "loopback";
  state.loopbackPeerId = peerId;
  const loopback = await setup({ service: () => Promise.resolve(state) });
  assert.equal(
    loopback.nodes.get("peer-avatar").querySelectorAll("svg")[0].dataset.icon,
    "bot",
  );
  assert.equal(
    loopback.nodes.get("contacts").querySelectorAll("svg")[0].dataset.icon,
    "bot",
  );
});

test("file cards accept the 1 GiB file boundary while images keep the separate 8 MiB limit", async () => {
  const state = withImages();
  state.messages[0].kind = "file";
  state.messages[0].attachment = {
    ...state.messages[0].attachment,
    name: "Archive.bin",
    mime: "application/octet-stream",
    bytes: 1024 * 1024 * 1024,
  };
  const env = await setup({
    service: () => Promise.resolve(structuredClone(state)),
  });
  assert.match(env.nodes.get("messages").textContent, /Archive\.bin.*1\.0 GiB/);
  state.messages[0].attachment.bytes++;
  env.nodes.get("refresh").onclick();
  await flush();
  assert.equal(env.nodes.get("status").textContent, "Refresh failed");
  assert.match(env.nodes.get("messages").textContent, /Archive\.bin.*1\.0 GiB/);
  state.messages = withImages().messages;
  state.messages[0].attachment.bytes = 8 * 1024 * 1024 + 1;
  env.nodes.get("refresh").onclick();
  await flush();
  assert.equal(env.nodes.get("status").textContent, "Refresh failed");
  assert.match(env.nodes.get("messages").textContent, /Archive\.bin/);
});

test("attachment errors use stable codes and distinguish size, cache, transport and storage failures", async () => {
  for (const [code, kind, expected] of [
    ["EXTENSION_ATTACHMENT_TOO_LARGE", "image", /8 MiB/],
    ["EXTENSION_ATTACHMENT_TOO_LARGE", "file", /1 GiB/],
    ["EXTENSION_ATTACHMENT_CACHE_FULL", "file", /cache is full.*2 GiB/],
    [
      "EXTENSION_TRANSFER_INTERRUPTED",
      "file",
      /interrupted or failed verification/,
    ],
    ["EXTENSION_TIMEOUT", "file", /timed out/],
    ["EXTENSION_BUSY", "file", /service is busy/],
    ["EXTENSION_IMAGE_INVALID", "image", /PNG, JPEG or WebP/],
    ["EXTENSION_IMAGE_TOO_LARGE", "image", /pixel or decoded-size limit/],
    ["STORAGE_IO", "file", /could not be read or saved/],
  ]) {
    const env = await setup({
      service: (request) =>
        request.action === "sendAttachment"
          ? Promise.reject({ code, message: "Untrusted error text" })
          : Promise.resolve(status()),
    });
    env.input("draft", "Keep my draft after failure");
    await env.nodes.get("send-" + kind).onclick();
    assert.match(env.notices.at(-1).message, expected);
    assert.doesNotMatch(env.notices.at(-1).message, /Untrusted error text/);
    if (code === "EXTENSION_TRANSFER_INTERRUPTED")
      assert.doesNotMatch(env.notices.at(-1).message, /MiB|GiB/);
    assert.equal(env.nodes.get("draft").value, "Keep my draft after failure");
    assert.equal(
      env.calls.some((request) => request.action === "clearHistory"),
      false,
    );
  }
});

test("stopping a transfer discards its late snapshot and cannot clear a newer operation's busy state", async () => {
  const sending = deferred(),
    stopping = deferred(),
    starting = deferred();
  const stopped = status();
  stopped.running = false;
  stopped.port = 0;
  stopped.addresses = [];
  stopped.peers[0].connected = false;
  const env = await setup({
    service: (request) =>
      request.action === "sendAttachment"
        ? sending.promise
        : request.action === "stop"
          ? stopping.promise
          : request.action === "start"
            ? starting.promise
            : Promise.resolve(status()),
  });
  env.input("draft", "Keep across cancellation");
  const send = env.nodes.get("send-file").onclick();
  assert.equal(env.nodes.get("cancel-transfer").hidden, false);
  assert.equal(env.nodes.get("stop").disabled, false);
  env.nodes.get("cancel-transfer").onclick();
  assert.equal(env.nodes.get("cancel-transfer").disabled, true);
  stopping.resolve(stopped);
  await flush();
  assert.equal(env.nodes.get("status").textContent, "Disconnected");
  assert.equal(env.nodes.get("draft").value, "Keep across cancellation");
  env.nodes.get("start").onclick();
  assert.equal(env.nodes.get("rename").disabled, true);
  sending.resolve(status());
  await send;
  await flush();
  assert.equal(env.nodes.get("rename").disabled, true);
  assert.equal(env.nodes.get("status").textContent, "Disconnected");
  const resumed = status();
  resumed.peers[0].connected = false;
  starting.resolve(resumed);
  await flush();
  assert.equal(env.nodes.get("rename").disabled, false);
  assert.equal(env.nodes.get("send").disabled, true);
  assert.match(env.nodes.get("peer-detail").textContent, /Offline/);
});

test("a failed stop never reports cancellation success and leaves an explicit retry available", async () => {
  const pending = deferred();
  const env = await setup({
    service: (request) =>
      request.action === "sendAttachment"
        ? pending.promise
        : request.action === "stop"
          ? Promise.reject({ code: "STORAGE_IO" })
          : Promise.resolve(status()),
  });
  env.input("draft", "Still retained");
  const sending = env.nodes.get("send-file").onclick();
  env.nodes.get("cancel-transfer").onclick();
  await flush();
  assert.match(env.notices.at(-1).message, /transfer result is unknown/);
  assert.equal(env.nodes.get("status").textContent, "Refresh failed");
  assert.equal(env.nodes.get("stop").disabled, false);
  assert.equal(env.nodes.get("draft").value, "Still retained");
  pending.resolve(status());
  await sending;
});

test("a shared four-request SDK retains a Stop slot during a thumbnail, transfer and unsaved checkpoint", async () => {
  const thumbnailRead = deferred(),
    transfer = deferred(),
    draftWrite = deferred();
  const state = withImages(2);
  const stopped = structuredClone(state);
  stopped.running = false;
  stopped.port = 0;
  stopped.addresses = [];
  stopped.peers[0].connected = false;
  const env = await setup({
    sdkLimit: 4,
    checkpoint: () => draftWrite.promise,
    service: (request) =>
      request.action === "previewAttachment"
        ? thumbnailRead.promise
        : request.action === "sendAttachment"
          ? transfer.promise
          : Promise.resolve(request.action === "stop" ? stopped : state),
  });
  assert.equal(
    env.calls.filter((request) => request.action === "previewAttachment")
      .length,
    1,
  );
  env.input("draft", "Initial unsaved draft");
  const sending = env.nodes.get("send-file").onclick();
  env.input("draft", "Latest unsaved draft");
  assert.equal(env.sdk.pending, 3);
  env.nodes.get("cancel-transfer").onclick();
  await flush();
  assert.equal(
    env.calls.filter((request) => request.action === "stop").length,
    1,
  );
  assert.equal(env.nodes.get("status").textContent, "Disconnected");
  assert.equal(env.nodes.get("draft").value, "Latest unsaved draft");
  assert.equal(env.sdk.maximumPending, 4);
  assert.deepEqual(env.sdk.rejected, []);
  transfer.resolve(state);
  thumbnailRead.resolve({ dataUrl: thumbnail });
  draftWrite.resolve();
  await sending;
  await flush();
  await env.tick(150);
  assert.equal(env.checkpoints.at(-1).value.draft, "Latest unsaved draft");
  assert.deepEqual(env.sdk.rejected, []);
  assert.equal(env.nodes.get("status").textContent, "Disconnected");
});

test("ordinary admission never queues a picker or connection to start after its user action was rejected", async () => {
  const thumbnailRead = deferred(),
    transfer = deferred();
  const state = withImages();
  const stopped = structuredClone(state);
  stopped.running = false;
  stopped.port = 0;
  stopped.addresses = [];
  stopped.peers[0].connected = false;
  const env = await setup({
    sdkLimit: 4,
    service: (request) =>
      request.action === "previewAttachment"
        ? thumbnailRead.promise
        : request.action === "sendAttachment"
          ? transfer.promise
          : Promise.resolve(request.action === "stop" ? stopped : state),
  });
  const sending = env.nodes.get("send-file").onclick();
  env.nodes.get("cancel-transfer").onclick();
  await flush();
  env.nodes.get("start").onclick();
  await flush();
  assert.equal(
    env.calls.filter((request) => request.action === "start").length,
    0,
  );
  transfer.resolve(state);
  thumbnailRead.resolve({ dataUrl: thumbnail });
  await sending;
  await flush();
  assert.equal(
    env.calls.filter((request) => request.action === "start").length,
    0,
  );
  assert.match(env.notices.at(-1).message, /service is busy/);
  assert.deepEqual(env.sdk.rejected, []);
});

test("a transient checkpoint conflict retries the retained draft without another edit", async () => {
  let attempts = 0;
  const env = await setup({
    checkpoint: () =>
      ++attempts === 1
        ? Promise.reject({ code: "CONFLICT" })
        : Promise.resolve(),
  });
  env.input("draft", "Only edit before a transient failure");
  await flush();
  assert.equal(env.checkpoints.length, 1);
  await env.tick(500);
  assert.equal(env.checkpoints.length, 2);
  assert.equal(
    env.checkpoints[1].value.draft,
    "Only edit before a transient failure",
  );
  assert.equal(env.checkpoints[1].dirty, true);
  await env.tick(1500);
  assert.equal(env.checkpoints.length, 2);
});

test("a failed older checkpoint cannot replace a newer queued draft", async () => {
  const first = deferred();
  let attempts = 0;
  const env = await setup({
    checkpoint: () => (++attempts === 1 ? first.promise : Promise.resolve()),
  });
  env.input("draft", "Older draft");
  env.input("draft", "Latest draft while the older write is in flight");
  first.reject({ code: "CONFLICT" });
  await flush();
  await env.tick(500);
  assert.equal(env.checkpoints.length, 2);
  assert.equal(
    env.checkpoints[1].value.draft,
    "Latest draft while the older write is in flight",
  );
});

test("a failed checkpoint is retained while inactive and saved when the page resumes", async () => {
  const first = deferred();
  let attempts = 0;
  const env = await setup({
    checkpoint: () => (++attempts === 1 ? first.promise : Promise.resolve()),
  });
  env.input("draft", "Retained across inactivity");
  env.emit({ active: false });
  first.reject({ code: "CONFLICT" });
  await flush();
  await env.tick(500);
  assert.equal(env.checkpoints.length, 1);
  env.emit({ active: true });
  await flush();
  assert.equal(env.checkpoints.length, 2);
  assert.equal(env.checkpoints[1].value.draft, "Retained across inactivity");
});

test("checkpoint retries are bounded, then a finished host save restarts the retained write", async () => {
  let failing = true;
  const env = await setup({
    checkpoint: () =>
      failing ? Promise.reject({ code: "CONFLICT" }) : Promise.resolve(),
  });
  env.emit({ busy: true });
  env.input("draft", "Edited while the host save dialog was open");
  await flush();
  for (const delay of [500, 1500, 5000]) await env.tick(delay);
  assert.equal(env.checkpoints.length, 4);
  for (const delay of [500, 1500, 5000]) await env.tick(delay);
  assert.equal(env.checkpoints.length, 4);
  assert.equal(env.notices.length, 1);
  failing = false;
  env.emit({ busy: false });
  await flush();
  assert.equal(env.checkpoints.length, 5);
  assert.equal(
    env.checkpoints.at(-1).value.draft,
    "Edited while the host save dialog was open",
  );
  assert.equal(env.checkpoints.at(-1).dirty, true);
});

test("Refresh retries a failed checkpoint immediately without leaving a duplicate timer", async () => {
  let failing = true;
  const env = await setup({
    checkpoint: () =>
      failing ? Promise.reject({ code: "CONFLICT" }) : Promise.resolve(),
  });
  env.input("draft", "Retry through Refresh");
  await flush();
  failing = false;
  await env.nodes.get("refresh").onclick();
  await flush();
  assert.equal(env.checkpoints.length, 2);
  assert.equal(env.checkpoints.at(-1).value.draft, "Retry through Refresh");
  await env.tick(500);
  assert.equal(env.checkpoints.length, 2);
});

test("forgetting a contact confirms draft deletion and checkpoints only that contact's removal", async () => {
  const state = status(),
    other = "d".repeat(32),
    pending = deferred();
  state.peers[0].connected = false;
  state.peers.push({
    ...state.peers[0],
    id: other,
    nickname: "Other friend",
    lastSeen: 1,
  });
  const env = await setup({
    service: (request) =>
      request.action === "forget"
        ? pending.promise
        : Promise.resolve(structuredClone(state)),
  });
  env.input("draft", "Draft to remove with this contact");
  env.nodes.get("contacts").querySelectorAll("button")[1].onclick();
  env.input("draft", "Keep the other conversation draft");
  env.nodes.get("contacts").querySelectorAll("button")[0].onclick();
  env.nodes.get("forget").onclick();
  assert.match(env.nodes.get("confirmation-text").textContent, /unsent draft/);
  const removing = env.nodes.get("confirm-action").onclick();
  assert.equal(env.nodes.get("draft").disabled, true);
  const result = structuredClone(state);
  result.peers = result.peers.filter((peer) => peer.id !== peerId);
  result.messages = [];
  pending.resolve(result);
  await removing;
  await env.tick(150);
  const saved = env.checkpoints.at(-1);
  assert.equal(saved.value.selected, other);
  assert.equal(Object.hasOwn(saved.value.drafts, peerId), false);
  assert.equal(saved.value.drafts[other], "Keep the other conversation draft");
  assert.equal(saved.dirty, true);
  assert.equal(
    env.nodes.get("draft").value,
    "Keep the other conversation draft",
  );
  assert.equal(env.nodes.get("draft").disabled, false);
});

test("a failed contact removal preserves its draft and restores editing", async () => {
  const state = status(),
    pending = deferred();
  state.peers[0].connected = false;
  const env = await setup({
    service: (request) =>
      request.action === "forget" ? pending.promise : Promise.resolve(state),
  });
  env.input("draft", "Keep if removing the contact fails");
  env.nodes.get("forget").onclick();
  const removing = env.nodes.get("confirm-action").onclick();
  assert.equal(env.nodes.get("draft").disabled, true);
  pending.reject({ code: "STORAGE_IO" });
  await removing;
  await env.tick(150);
  assert.equal(env.nodes.get("draft").disabled, false);
  assert.equal(
    env.nodes.get("draft").value,
    "Keep if removing the contact fails",
  );
  assert.equal(
    env.checkpoints.at(-1).value.drafts[peerId],
    "Keep if removing the contact fails",
  );
  assert.equal(env.checkpoints.at(-1).value.selected, peerId);
  assert.equal(env.nodes.get("contacts").querySelectorAll("button").length, 1);
});

test("an oversized newer draft invalidates older queued and failed checkpoint retries", async () => {
  const first = deferred(),
    chunk = "界".repeat(8192);
  const state = status();
  state.messages = [];
  state.peers = Array.from({ length: 10 }, (_, index) => ({
    ...state.peers[0],
    id: (index + 10).toString(16).padStart(32, "0"),
    nickname: "Friend " + index,
  }));
  const drafts = Object.fromEntries(
    state.peers.slice(0, 9).map((peer) => [peer.id, chunk]),
  );
  let attempts = 0;
  const env = await setup({
    saved: { version: 1, selected: state.peers[0].id, draft: chunk, drafts },
    checkpoint: () => (++attempts === 1 ? first.promise : Promise.resolve()),
    service: () => Promise.resolve(state),
  });
  env.input("draft", chunk);
  env.nodes.get("contacts").querySelectorAll("button")[9].onclick();
  env.input("draft", chunk);
  await flush();
  assert.match(env.notices.at(-1).message, /Too many unsent drafts/);
  first.reject({ code: "CONFLICT" });
  await flush();
  for (const delay of [150, 500, 1500, 5000]) await env.tick(delay);
  assert.equal(env.checkpoints.length, 1);
  assert.equal(env.nodes.get("draft").value, chunk);
  env.input("draft", "");
  await flush();
  assert.equal(env.checkpoints.length, 2);
  assert.equal(
    Object.hasOwn(env.checkpoints.at(-1).value.drafts, state.peers[9].id),
    false,
  );
  assert.equal(env.checkpoints.at(-1).dirty, true);
  env.input("draft", chunk);
  await flush();
  assert.equal(env.checkpoints.length, 2);
  assert.equal(
    env.notices.filter((notice) =>
      /Too many unsent drafts/.test(notice.message),
    ).length,
    2,
  );
});

test("numeric port drafts round-trip through real checkpoints and can be corrected before connecting", async () => {
  for (const value of ["", "0", "70000", "-1", "0.5", "1e2", "6.5535e4"]) {
    const stopped = status();
    stopped.running = false;
    stopped.port = 0;
    stopped.peers = [];
    stopped.messages = [];
    const env = await setup({ service: () => Promise.resolve(stopped) });
    env.input("draft", "Keep the unsubmitted connection draft");
    for (const id of ["port", "remote-port"]) {
      env.nodes.get(id).value = value;
      env.nodes.get(id).onchange();
    }
    await flush();
    await env.tick(150);
    const saved = env.checkpoints.at(-1).value;
    assert.equal(saved.port, value);
    assert.equal(saved.remotePort, value);
    const started = { ...stopped, running: true, port: 43123 };
    const reopened = await setup({
      saved,
      service: (request) =>
        Promise.resolve(request.action === "status" ? stopped : started),
    });
    assert.equal(reopened.nodes.get("draft").disabled, false);
    assert.equal(
      reopened.nodes.get("draft").value,
      "Keep the unsubmitted connection draft",
    );
    assert.equal(reopened.nodes.get("port").value, value);
    assert.equal(reopened.nodes.get("remote-port").value, value);
    if (![100, 65535].includes(Number(value))) {
      reopened.nodes.get("start").onclick();
      await flush();
      assert.equal(
        reopened.calls.filter((request) => request.action === "start").length,
        0,
      );
    }
    reopened.nodes.get("port").value = "43123";
    reopened.nodes.get("start").onclick();
    await flush();
    assert.equal(
      reopened.calls.find((request) => request.action === "start").port,
      43123,
    );
    assert.equal(reopened.nodes.get("status").textContent, "LAN online");
    reopened.nodes.get("remote-port").value = "43124";
    reopened.nodes.get("address").value = "127.0.0.1";
    reopened.nodes.get("connect").onclick();
    await flush();
    const connected = reopened.calls.find(
      (request) => request.action === "connect",
    );
    assert.equal(connected.port, 43124);
    assert.equal(connected.address, "127.0.0.1");
  }
});

test("unsubmitted address text and the maximum nickname round-trip without poisoning the checkpoint", async () => {
  for (const address of ["a".repeat(101), "界".repeat(34)]) {
    const env = await setup();
    env.nodes.get("address").value = address;
    env.nodes.get("address").onchange();
    const nickname = "名".repeat(40);
    env.input("nickname", nickname);
    await flush();
    await env.tick(150);
    const saved = env.checkpoints.at(-1).value;
    const reopened = await setup({ saved });
    assert.equal(reopened.nodes.get("draft").disabled, false);
    assert.equal(reopened.nodes.get("address").value, address);
    assert.equal(reopened.nodes.get("nickname").value, nickname);
    assert.equal(
      reopened.notices.some((notice) =>
        /cannot be restored/.test(notice.message),
      ),
      false,
    );
  }
});
