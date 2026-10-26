import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const html = await readFile(new URL("./main.html", import.meta.url), "utf8");
const source = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const initial = () => ({
  schema: 1,
  seed: 20261026,
  difficulty: "normal",
  wrap: false,
  obstacles: false,
  moves: [],
  muted: false,
});
const settle = () => new Promise(setImmediate);
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
};

function fixture({
  checkpoint = null,
  state,
  onCheckpoint,
  onRun,
  view = {},
} = {}) {
  class Element {
    constructor() {
      this.listeners = new Map();
      this.attributes = new Map();
      this.dataset = {};
      this.hidden = false;
      this.disabled = false;
      this.writes = 0;
      this.content = "";
    }
    get textContent() {
      return this.content;
    }
    set textContent(value) {
      this.writes++;
      this.content = value;
    }
    addEventListener(name, listener) {
      const callbacks = this.listeners.get(name) ?? new Set();
      callbacks.add(listener);
      this.listeners.set(name, callbacks);
    }
    removeEventListener(name, listener) {
      this.listeners.get(name)?.delete(listener);
    }
    dispatch(name, event = {}) {
      const value = { target: this, preventDefault() {}, ...event };
      for (const listener of this.listeners.get(name) ?? []) listener(value);
    }
    setAttribute(name, value) {
      this.attributes.set(name, value);
    }
    removeAttribute(name) {
      this.attributes.delete(name);
    }
    focus() {
      document.activeElement = this;
    }
    setPointerCapture() {}
  }
  class Input extends Element {}
  class Button extends Element {}
  class Select extends Element {}
  class Textarea extends Element {}
  const nodes = new Map();
  for (const [, tag, id] of html.matchAll(
    /<(\w+)\b[^>]*\bid="([^"]+)"[^>]*>/g,
  )) {
    const Type =
      { input: Input, button: Button, select: Select }[tag] ?? Element;
    nodes.set(id, new Type());
  }
  const $ = (id) => nodes.get(id);
  const directions = ["up", "right", "down", "left"].map((direction) => {
    const button = new Button();
    button.dataset.direction = direction;
    return button;
  });
  const document = new Element();
  document.documentElement = new Element();
  document.hidden = false;
  document.getElementById = $;
  document.querySelector = () => main;
  document.querySelectorAll = () => directions;
  const main = new Element();
  $("confirm").hidden = true;
  $("speed").value = "normal";
  $("speed").options = [new Element(), new Element(), new Element()];
  $("wrap").checked = false;
  $("rocks").checked = false;
  $("seed").value = "20261026";
  $("board").clientWidth = 420;
  $("board").getContext = () =>
    new Proxy({}, { get: (target, key) => target[key] ?? (() => {}) });
  const frames = new Map();
  let frameId = 0,
    styles = 0,
    listener,
    visibility;
  const checkpoints = [],
    runs = [],
    notifications = [];
  const window = new Element();
  const starter = {
    onChange(callback) {
      listener = callback;
      callback(snapshot);
      return () => {
        listener = null;
      };
    },
    checkpoint(value, dirty) {
      checkpoints.push({ value: structuredClone(value), dirty });
      return onCheckpoint?.(value, dirty) ?? Promise.resolve();
    },
    run(action, fields) {
      runs.push({ action, fields });
      return onRun?.(action, fields) ?? Promise.resolve(snapshot.view);
    },
    notify(message, options) {
      notifications.push({ message, ...options });
      return Promise.resolve();
    },
  };
  let snapshot = {
    locale: "en",
    busy: false,
    grantedPermissions: ["storage"],
    checkpoint,
    view: {
      state: state ?? JSON.stringify({ schema: 1, game: initial() }),
      description: "Ready",
      blocks: [{ kind: "table", rows: [] }],
      groups: [{ actions: [{ id: "save-best", disabled: false }] }],
      ...view,
    },
  };
  vm.runInNewContext(source, {
    document,
    window,
    starter,
    structuredClone,
    HTMLInputElement: Input,
    HTMLButtonElement: Button,
    HTMLSelectElement: Select,
    HTMLTextAreaElement: Textarea,
    matchMedia: () => new Element(),
    requestAnimationFrame(callback) {
      frames.set(++frameId, callback);
      return frameId;
    },
    cancelAnimationFrame(id) {
      frames.delete(id);
    },
    ResizeObserver: class {
      observe() {}
      disconnect() {}
    },
    IntersectionObserver: class {
      constructor(callback) {
        visibility = callback;
      }
      observe() {}
      disconnect() {}
    },
    getComputedStyle: () => {
      styles++;
      return { getPropertyValue: () => "#333333" };
    },
    devicePixelRatio: 1,
    setTimeout,
    clearTimeout,
  });
  return {
    $,
    document,
    window,
    frames,
    directions,
    checkpoints,
    runs,
    notifications,
    get styles() {
      return styles;
    },
    get snapshot() {
      return snapshot;
    },
    emit(value) {
      snapshot = { ...snapshot, ...value };
      listener?.(snapshot);
    },
    click(id) {
      if (!$(id).disabled) $(id).dispatch("click");
    },
    key(key, target = $("board"), other = {}) {
      document.dispatch("keydown", {
        key,
        code: key === " " ? "Space" : key,
        target,
        ...other,
      });
    },
    advance(now) {
      const callbacks = [...frames.values()];
      frames.clear();
      for (const callback of callbacks) callback(now);
    },
    visible(isIntersecting) {
      visibility([{ isIntersecting }]);
    },
    dispose() {
      window.dispatch("pagehide");
    },
  };
}

test("slow checkpoint acknowledgements coalesce moves into one latest draft", async (t) => {
  const first = deferred();
  let count = 0;
  const app = fixture({
    onCheckpoint: () => (++count === 1 ? first.promise : Promise.resolve()),
  });
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  for (const now of [1, 111, 221, 331, 441]) app.advance(now);
  await settle();
  assert.equal(app.checkpoints.length, 1);
  first.resolve();
  await settle();
  assert.equal(app.checkpoints.length, 2);
  assert.deepEqual(app.checkpoints[1].value.moves, [
    "right",
    "right",
    "right",
    "right",
  ]);
  assert.equal(app.checkpoints[1].dirty, false);
});

for (const order of ["before", "after"]) {
  test(`a checkpoint CONFLICT arriving ${order} busy snapshots retries once after idle`, async (t) => {
    const first = deferred();
    let count = 0;
    const app = fixture({
      onCheckpoint: () => (++count === 1 ? first.promise : Promise.resolve()),
    });
    t.after(() => app.dispose());
    app.click("start");
    await settle();
    app.advance(1);
    app.advance(111);
    if (order === "after") {
      app.emit({ busy: true });
      app.emit({ busy: false });
    }
    first.reject(Object.assign(Error("busy"), { code: "CONFLICT" }));
    await settle();
    if (order === "before") {
      assert.equal(app.checkpoints.length, 1);
      app.emit({ busy: true });
      app.emit({ busy: false });
      await settle();
    }
    assert.equal(app.checkpoints.length, 2);
    assert.deepEqual(app.checkpoints.at(-1).value.moves, ["right"]);
    app.emit({ busy: true });
    app.emit({ busy: false });
    await settle();
    assert.equal(app.checkpoints.length, 2);
    assert.equal(app.$("overlay").hidden, false);
  });
}

test("a second conflict does not retry without a user action", async (t) => {
  const app = fixture({
    onCheckpoint: () =>
      Promise.reject(Object.assign(Error("busy"), { code: "CONFLICT" })),
  });
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  app.emit({ busy: true });
  app.emit({ busy: false });
  await settle();
  app.emit({ busy: true });
  app.emit({ busy: false });
  await settle();
  assert.equal(app.checkpoints.length, 2);
  assert.equal(app.frames.size, 0);
  app.click("save");
  await settle();
  assert.equal(app.checkpoints.length, 3);
  assert.equal(app.runs.length, 0);
  assert.match(app.notifications.at(-1).message, /Save failed/);
});

test("ordinary checkpoint failures stay paused and saving does not run Wasm", async (t) => {
  const app = fixture({
    onCheckpoint: () => Promise.reject(Error("connection failed")),
  });
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  app.emit({ busy: true });
  app.emit({ busy: false });
  await settle();
  assert.equal(app.checkpoints.length, 1);
  assert.equal(app.frames.size, 0);
  app.click("save");
  await settle();
  assert.equal(app.checkpoints.length, 2);
  assert.equal(app.runs.length, 0);
});

test("busy operations pause gameplay and defer the latest checkpoint until idle", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  app.advance(1);
  app.advance(111);
  app.emit({ busy: true });
  await settle();
  assert.equal(app.checkpoints.length, 1);
  assert.equal(app.$("start").disabled, true);
  assert.equal(app.$("restart").disabled, true);
  assert.equal(app.$("sound").disabled, true);
  app.emit({ busy: false });
  await settle();
  assert.deepEqual(app.checkpoints.at(-1).value.moves, ["right"]);
  assert.equal(app.$("overlay").hidden, false);
  assert.equal(app.frames.size, 0);
});

test("invalid imported checkpoints remain untouched until a confirmed new game", async (t) => {
  const app = fixture({ checkpoint: { schema: 9, original: "preserve this" } });
  t.after(() => app.dispose());
  assert.equal(app.$("data-error").hidden, false);
  assert.match(app.$("data-error").textContent, /preserved/);
  app.click("sound");
  app.document.hidden = true;
  app.document.dispatch("visibilitychange");
  app.document.hidden = false;
  app.click("start");
  await settle();
  assert.equal(app.checkpoints.length, 0);
  assert.equal(app.$("confirm").hidden, false);
  app.key("Escape");
  assert.equal(app.$("confirm").hidden, true);
  assert.equal(app.document.activeElement, app.$("restart"));
  app.click("restart");
  app.click("confirm-restart");
  await settle();
  assert.equal(app.checkpoints.length, 1);
  assert.equal(app.checkpoints[0].value.schema, 1);
  assert.equal(app.$("data-error").hidden, true);
  assert.equal(app.document.activeElement, app.$("start"));
});

test("a valid checkpoint can recover even when the old Wasm state cannot be decoded", (t) => {
  const app = fixture({
    checkpoint: { ...initial(), moves: ["up"] },
    state: "invalid old state",
  });
  t.after(() => app.dispose());
  assert.equal(app.$("data-error").hidden, true);
  assert.equal(app.$("start").textContent, "Resume");
});

test("checkpoint replay keeps the deterministic food sequence and rejects non-string moves", (t) => {
  const app = fixture({
    checkpoint: {
      ...initial(),
      moves: [...Array(8).fill("up"), ...Array(6).fill("left")],
    },
  });
  t.after(() => app.dispose());
  assert.equal(app.$("score").textContent, "10");
  assert.equal(app.$("data-error").hidden, true);
  const invalid = fixture({ checkpoint: { ...initial(), moves: [["up"]] } });
  t.after(() => invalid.dispose());
  assert.equal(invalid.$("data-error").hidden, false);
});

test("read-only record warnings do not block offline gameplay", async (t) => {
  const app = fixture({
    view: {
      groups: [{ actions: [{ id: "save-best", disabled: true }] }],
      blocks: [
        { kind: "table", rows: [] },
        { kind: "markdown", text: "Future records are preserved." },
      ],
    },
  });
  t.after(() => app.dispose());
  assert.equal(app.$("save").disabled, true);
  assert.equal(app.$("best").textContent, "—");
  assert.equal(
    app.$("data-error").textContent,
    "Future records are preserved.",
  );
  app.click("start");
  await settle();
  assert.equal(app.$("overlay").hidden, true);
  assert.equal(app.runs.length, 0);
});

test("saving locks board changes and keeps the exact checkpoint submitted", async (t) => {
  const result = deferred();
  const app = fixture({ onRun: () => result.promise });
  t.after(() => app.dispose());
  app.click("save");
  await settle();
  assert.equal(app.runs.length, 1);
  assert.equal(app.$("sound").disabled, true);
  app.$("sound").dispatch("click");
  app.$("confirm-restart").dispatch("click");
  result.resolve(app.snapshot.view);
  await settle();
  assert.deepEqual(JSON.parse(app.runs[0].fields.checkpoint), initial());
  assert.equal(app.$("save").disabled, false);
  assert.equal(app.$("overlay").hidden, false);
});

test("direction-button focus keeps WASD usable and editing keys retain their behavior", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  app.advance(1);
  app.key("w", app.directions[0]);
  app.key("a", app.$("seed"));
  app.advance(111);
  await settle();
  assert.deepEqual(app.checkpoints.at(-1).value.moves, ["up"]);
  app.key("Escape");
  assert.equal(app.$("overlay").hidden, true);
});

test("secondary pointers cannot replace the active swipe and offscreen play pauses", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  app.advance(1);
  app.$("board").dispatch("pointerdown", {
    pointerId: 1,
    isPrimary: true,
    button: 0,
    clientX: 100,
    clientY: 100,
  });
  app.$("board").dispatch("pointerdown", {
    pointerId: 2,
    isPrimary: false,
    button: 0,
    clientX: 20,
    clientY: 20,
  });
  app
    .$("board")
    .dispatch("pointerup", { pointerId: 1, clientX: 100, clientY: 50 });
  app.advance(111);
  await settle();
  assert.deepEqual(app.checkpoints.at(-1).value.moves, ["up"]);
  app.visible(false);
  assert.equal(app.frames.size, 0);
  app.click("start");
  assert.equal(app.frames.size, 0);
});

test("language changes preserve gameplay, status text avoids tick mutations and drawing reads few styles", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.click("start");
  await settle();
  const writes = app.$("notice").writes;
  const before = app.styles;
  app.advance(1);
  app.advance(111);
  await settle();
  assert.equal(app.$("notice").writes, writes);
  assert.ok(app.styles - before < 10);
  app.emit({ locale: "zh-CN" });
  assert.equal(app.document.title, "贪吃蛇");
  assert.equal(app.$("overlay").hidden, true);
  assert.deepEqual(app.checkpoints.at(-1).value.moves, ["right"]);
});
