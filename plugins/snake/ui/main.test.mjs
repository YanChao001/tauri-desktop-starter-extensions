import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const html = await readFile(new URL("./main.html", import.meta.url), "utf8");
const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(
  ([, source]) => source,
);
const initial = (changes = {}) => ({
  schema: 3,
  seed: 20261026,
  difficulty: "normal",
  skin: 0,
  muted: false,
  volume: 65,
  nickname: "",
  inputs: [],
  ...changes,
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
  onAudioResume,
  onAudioSuspend,
  onAudioDecode,
  onGamepads,
  onReplayYield,
  fakeAudioTimers = false,
  view = {},
} = {}) {
  class Element {
    constructor(tagName = "div") {
      this.tagName = tagName.toUpperCase();
      this.listeners = new Map();
      this.attributes = new Map();
      this.dataset = {};
      this.style = { setProperty() {}, removeProperty() {} };
      const classes = new Set();
      this.classList = {
        add: (...values) => values.forEach((value) => classes.add(value)),
        remove: (...values) => values.forEach((value) => classes.delete(value)),
        contains: (value) => classes.has(value),
        toggle: (value, force) => {
          const enabled = force ?? !classes.has(value);
          if (enabled) classes.add(value);
          else classes.delete(value);
          return enabled;
        },
      };
      this.hidden = false;
      this.disabled = false;
      this.inert = false;
      this.writes = 0;
      this.content = "";
      this.children = [];
      this.parentElement = null;
      this.value = "";
      this.clientWidth = 960;
      this.clientHeight = 600;
    }
    get textContent() {
      return (
        this.content + this.children.map((child) => child.textContent).join("")
      );
    }
    set textContent(value) {
      this.writes++;
      this.content = String(value);
      this.children = [];
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
      const value = {
        target: this,
        currentTarget: this,
        preventDefault() {},
        ...event,
      };
      for (const listener of this.listeners.get(name) ?? []) listener(value);
      return value;
    }
    click() {
      if (!this.disabled) this.dispatch("click");
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) ?? null;
    }
    removeAttribute(name) {
      this.attributes.delete(name);
    }
    append(...children) {
      for (const child of children) child.parentElement = this;
      this.children.push(...children);
    }
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      return child;
    }
    replaceChildren(...children) {
      this.content = "";
      this.children = [...children];
    }
    focus() {
      document.activeElement = this;
    }
    contains(node) {
      return (
        node === this || this.children.some((child) => child.contains(node))
      );
    }
    closest(selector) {
      const matches = selector.split(",").some((part) => {
        const value = part.trim();
        return (
          value === this.tagName.toLowerCase() ||
          (value === "[tabindex]" && this.getAttribute("tabindex") !== null)
        );
      });
      return matches ? this : (this.parentElement?.closest(selector) ?? null);
    }
    querySelectorAll() {
      return this.children;
    }
    getBoundingClientRect() {
      return {
        x: 0,
        y: 0,
        left: 0,
        top: 0,
        width: this.clientWidth,
        height: this.clientHeight,
        right: this.clientWidth,
        bottom: this.clientHeight,
      };
    }
    setPointerCapture() {}
    releasePointerCapture() {}
  }
  class Input extends Element {}
  class Button extends Element {}
  class Select extends Element {}
  class Textarea extends Element {}
  const nodes = new Map();
  for (const [markup, tag, id] of html.matchAll(
    /<(\w+)\b[^>]*\bid="([^"]+)"[^>]*>/g,
  )) {
    const Type =
      { input: Input, button: Button, select: Select }[tag] ?? Element;
    const element = new Type(tag);
    element.id = id;
    element.hidden = /\bhidden(?:\s|>|=)/.test(markup);
    element.value = markup.match(/\bvalue="([^"]*)"/)?.[1] ?? "";
    for (const [, name, value] of markup.matchAll(/\b([\w-]+)="([^"]*)"/g))
      element.setAttribute(name, value);
    nodes.set(id, element);
  }
  const $ = (id) => {
    if (nodes.has(id)) return nodes.get(id);
    const find = (node) =>
      node.id === id ? node : node.children.map(find).find(Boolean);
    return [...nodes.values()].map(find).find(Boolean);
  };
  const document = new Element("document");
  document.documentElement = new Element("html");
  document.body = new Element("body");
  document.hidden = false;
  document.getElementById = $;
  document.createElement = (tag) => new Element(tag);
  const main = new Element("main");
  document.querySelector = (selector) =>
    selector.startsWith("#") ? $(selector.slice(1)) : main;
  document.querySelectorAll = (selector) => {
    if (selector === "button")
      return [...nodes.values()].filter((node) => node.tagName === "BUTTON");
    return [];
  };
  if ($("difficulty")) {
    $("difficulty").value = "normal";
    $("difficulty").options = [
      new Element("option"),
      new Element("option"),
      new Element("option"),
    ];
  }
  if ($("volume")) $("volume").value = "65";
  const drawnText = [];
  const context2d = new Proxy(
    {
      createLinearGradient: () => ({ addColorStop() {} }),
      createRadialGradient: () => ({ addColorStop() {} }),
      measureText: (text) => ({ width: String(text).length * 8 }),
      fillText(text, x, y) {
        drawnText.push({
          text: String(text),
          x,
          y,
          font: this.font,
          textAlign: this.textAlign,
          textBaseline: this.textBaseline,
        });
      },
    },
    { get: (target, key) => target[key] ?? (() => {}) },
  );
  for (const node of nodes.values())
    if (node.tagName === "CANVAS") node.getContext = () => context2d;
  const frames = new Map();
  const audioTimers = new Map();
  let frameId = 0,
    styles = 0,
    now = 0,
    listener,
    visibility;
  const checkpoints = [],
    runs = [],
    notifications = [],
    audios = [];
  const window = new Element("window");
  class AudioContext {
    constructor() {
      this.state = "suspended";
      this.currentTime = 1;
      this.destination = {};
      this.sources = [];
      this.gains = [];
      this.decoded = [];
      this.suspensions = 0;
      this.resumptions = 0;
      this.closed = false;
      audios.push(this);
    }
    resume() {
      this.resumptions++;
      return Promise.resolve(onAudioResume?.(this, this.resumptions)).then(
        () => {
          if (!this.closed) this.state = "running";
        },
      );
    }
    suspend() {
      this.suspensions++;
      const pending = onAudioSuspend?.(this, this.suspensions);
      if (!pending) {
        this.state = "suspended";
        return Promise.resolve();
      }
      return Promise.resolve(pending).then(() => {
        if (!this.closed) this.state = "suspended";
      });
    }
    close() {
      this.state = "closed";
      this.closed = true;
      return Promise.resolve();
    }
    async decodeAudioData(data) {
      this.decoded.push(data);
      await onAudioDecode?.(data);
      return { duration: 0.25, bytes: data.byteLength, data };
    }
    createBufferSource() {
      const source = {
        buffer: null,
        loop: false,
        playbackRate: { value: 1 },
        connect() {},
        disconnect() {
          this.disconnected = true;
        },
        start(time = 0) {
          this.started = time;
        },
        stop(time = 0) {
          this.stopped = time;
        },
      };
      this.sources.push(source);
      return source;
    }
    createGain() {
      const gain = {
        gain: {
          value: 1,
          setValueAtTime(value) {
            this.value = value;
          },
          linearRampToValueAtTime(value) {
            this.value = value;
          },
        },
        connect() {},
        disconnect() {
          this.disconnected = true;
        },
      };
      this.gains.push(gain);
      return gain;
    }
  }
  window.AudioContext = AudioContext;
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
      runs.push({ action, fields: structuredClone(fields) });
      return onRun?.(action, fields) ?? Promise.resolve(snapshot.view);
    },
    notify(message, options) {
      notifications.push({ message, ...options });
      return Promise.resolve();
    },
  };
  let snapshot = {
    locale: "en",
    active: true,
    busy: false,
    grantedPermissions: ["storage"],
    checkpoint,
    view: {
      state: state ?? JSON.stringify({ schema: 3, game: initial() }),
      description: "Ready",
      blocks: [{ kind: "table", rows: [] }],
      groups: [{ actions: [{ id: "save-best", disabled: false }] }],
      ...view,
    },
  };
  const context = vm.createContext({
    document,
    window,
    AudioContext,
    starter,
    structuredClone,
    navigator: { getGamepads: () => onGamepads?.() ?? [] },
    Image: class {
      complete = false;
      naturalWidth = 0;
    },
    HTMLInputElement: Input,
    HTMLButtonElement: Button,
    HTMLSelectElement: Select,
    HTMLTextAreaElement: Textarea,
    atob: (data) => Buffer.from(data, "base64").toString("binary"),
    crypto: {
      getRandomValues: (array) => {
        array.fill(20261026);
        return array;
      },
    },
    performance: { now: () => now },
    matchMedia: () => Object.assign(new Element(), { matches: false }),
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
    setTimeout: (callback, delay) => {
      if (delay === 0 && onReplayYield) {
        onReplayYield(callback);
        return 0;
      }
      if (delay === 600 && fakeAudioTimers) {
        const timer = { callback, delay };
        audioTimers.set(timer, callback);
        return timer;
      }
      return setTimeout(callback, delay);
    },
    clearTimeout: (timer) => {
      if (audioTimers.has(timer)) audioTimers.delete(timer);
      else clearTimeout(timer);
    },
  });
  for (const source of scripts) vm.runInContext(source, context);
  return {
    $,
    document,
    window,
    frames,
    checkpoints,
    runs,
    notifications,
    audios,
    audioTimers,
    drawnText,
    assets: vm.runInContext("SLITHER_ASSETS", context),
    engine: context.SlitherEngine,
    get styles() {
      return styles;
    },
    get writes() {
      return [...nodes.values()].reduce((sum, node) => sum + node.writes, 0);
    },
    get snapshot() {
      return snapshot;
    },
    finishAudioTail() {
      const callbacks = [...audioTimers.values()];
      audioTimers.clear();
      for (const callback of callbacks) callback();
    },
    emit(value) {
      snapshot = { ...snapshot, ...value };
      listener?.(snapshot);
    },
    click(id) {
      $(id).click();
    },
    key(key, target = $("board"), other = {}) {
      document.dispatch("keydown", {
        key,
        code: key === " " ? "Space" : key,
        target,
        ...other,
      });
    },
    keyup(key, target = $("board"), other = {}) {
      document.dispatch("keyup", {
        key,
        code: key === " " ? "Space" : key,
        target,
        ...other,
      });
    },
    advance(value) {
      now = value;
      for (const audio of audios) audio.currentTime = 1 + now / 1000;
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

const tickCount = (checkpoint) =>
  checkpoint.inputs.reduce((sum, input) => sum + input[2], 0);
const latest = (app) => app.checkpoints.at(-1)?.value;
const start = async (app) => {
  await settle();
  app.click("menu-start");
  await settle();
};

test("the arena waits for a gesture and checkpoints a compact 20 Hz input log", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.$("game-screen").hidden, true);
  assert.equal(app.frames.size, 0);
  assert.equal(app.audios.length, 0);
  assert.equal(app.checkpoints.length, 0);
  await start(app);
  assert.equal(app.$("game-screen").hidden, false);
  assert.equal(app.$("overlay").hidden, true);
  for (const now of [1, 51, 101, 151, 201, 251]) app.advance(now);
  await settle();
  app.click("pause");
  await settle();
  assert.equal(app.frames.size, 0);
  assert.equal(app.$("overlay").hidden, false);
  assert.equal(latest(app).schema, 3);
  assert.equal(tickCount(latest(app)), 5);
  assert.equal(latest(app).inputs.length, 1);
  assert.equal(latest(app).inputs[0][1], false);
  assert.equal(app.checkpoints.at(-1).dirty, false);
  assert.equal(app.runs.length, 0);
});

test("slow checkpoint acknowledgements coalesce the newest continuous arena input", async (t) => {
  const pending = deferred();
  let count = 0;
  const app = fixture({
    onCheckpoint: () => (++count === 1 ? pending.promise : Promise.resolve()),
  });
  t.after(() => app.dispose());
  await start(app);
  for (const now of [1, 51, 101, 151, 201, 251, 301, 351]) app.advance(now);
  app.click("pause");
  await settle();
  assert.equal(app.checkpoints.length, 1);
  pending.resolve();
  await settle();
  assert.equal(app.checkpoints.length, 2);
  assert.equal(tickCount(latest(app)), 7);
  assert.equal(latest(app).inputs.length, 1);
});

for (const order of ["before", "after"]) {
  test(`checkpoint conflicts arriving ${order} busy snapshots retry once after idle`, async (t) => {
    const pending = deferred();
    let count = 0;
    const app = fixture({
      onCheckpoint: () => (++count === 1 ? pending.promise : Promise.resolve()),
    });
    t.after(() => app.dispose());
    await start(app);
    app.advance(1);
    app.advance(51);
    if (order === "after") {
      app.emit({ busy: true });
      app.emit({ busy: false });
    }
    pending.reject(Object.assign(Error("busy"), { code: "CONFLICT" }));
    await settle();
    if (order === "before") {
      assert.equal(app.checkpoints.length, 1);
      app.emit({ busy: true });
      app.emit({ busy: false });
      await settle();
    }
    assert.equal(app.checkpoints.length, 2);
    assert.equal(tickCount(latest(app)), 1);
    app.emit({ busy: true });
    app.emit({ busy: false });
    await settle();
    assert.equal(app.checkpoints.length, 2);
    assert.equal(app.frames.size, 0);
    assert.equal(app.$("overlay").hidden, false);
  });
}

test("repeated checkpoint conflicts require a user retry and never submit an unacknowledged save", async (t) => {
  const app = fixture({
    onCheckpoint: () =>
      Promise.reject(Object.assign(Error("busy"), { code: "CONFLICT" })),
  });
  t.after(() => app.dispose());
  await start(app);
  for (let index = 0; index < 2; index++) {
    app.emit({ busy: true });
    app.emit({ busy: false });
    await settle();
  }
  assert.equal(app.checkpoints.length, 2);
  assert.equal(app.frames.size, 0);
  app.click("save");
  await settle();
  assert.equal(app.checkpoints.length, 3);
  assert.equal(app.runs.length, 0);
  assert.match(app.notifications.at(-1).message, /did not complete/i);
});

test("ordinary checkpoint failures stay paused and do not retry on unrelated host updates", async (t) => {
  const app = fixture({
    onCheckpoint: () => Promise.reject(Error("connection failed")),
  });
  t.after(() => app.dispose());
  await start(app);
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

test("host operations pause and lock the arena, then flush its exact latest input on idle", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.advance(51);
  app.emit({ busy: true });
  await settle();
  const writes = app.checkpoints.length;
  assert.equal(app.$("start").disabled, true);
  assert.equal(app.$("restart").disabled, true);
  assert.equal(app.$("sound").disabled, true);
  assert.equal(app.frames.size, 0);
  app.emit({ busy: false });
  await settle();
  assert.ok(app.checkpoints.length > writes);
  assert.equal(tickCount(latest(app)), 1);
  assert.equal(app.$("overlay").hidden, false);
  assert.equal(app.frames.size, 0);
});

for (const checkpoint of [
  { schema: 2, moves: ["up"], original: "preserve old grid game" },
  initial({ schema: 9, original: "preserve future game" }),
  initial({ inputs: [["east", false, 1]] }),
]) {
  test(`unsupported or corrupt checkpoint schema ${checkpoint.schema} is preserved until a confirmed new arena`, async (t) => {
    const app = fixture({ checkpoint });
    t.after(() => app.dispose());
    await settle();
    assert.equal(app.$("data-error").hidden, false);
    assert.match(app.$("data-error").textContent, /preserv/i);
    app.click("sound");
    app.$("volume").value = "20";
    app.$("volume").dispatch("input");
    app.document.hidden = true;
    app.document.dispatch("visibilitychange");
    app.document.hidden = false;
    app.click("menu-start");
    await settle();
    assert.equal(app.checkpoints.length, 0);
    assert.equal(app.$("confirm").hidden, false);
    app.click("confirm-restart");
    await settle();
    assert.equal(latest(app).schema, 3);
    assert.equal(app.$("data-error").hidden, true);
  });
}

test("a valid checkpoint recovers independently of an undecodable old Wasm view and waits paused", async (t) => {
  const checkpoint = initial({
    inputs: [[13, false, 3]],
    nickname: "Pip",
    skin: 4,
    volume: 28,
  });
  const app = fixture({ checkpoint, state: "invalid old state" });
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.$("data-error").hidden, true);
  assert.equal(app.frames.size, 0);
  assert.equal(app.checkpoints.length, 0);
  assert.equal(app.$("nickname").value, "Pip");
  assert.equal(app.$("volume").value, "28");
  await start(app);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app), checkpoint);
});

test("read-only record warnings leave offline controls available", async (t) => {
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
  await settle();
  assert.equal(app.$("save").disabled, true);
  assert.match(app.$("data-error").textContent, /Future records/);
  await start(app);
  assert.equal(app.$("overlay").hidden, true);
  assert.equal(app.frames.size, 1);
  assert.equal(app.runs.length, 0);
});

test("saving locks settings and submits exactly the acknowledged schema 3 checkpoint", async (t) => {
  const pending = deferred();
  const app = fixture({ onRun: () => pending.promise });
  t.after(() => app.dispose());
  await settle();
  app.click("save");
  await settle();
  assert.equal(app.runs.length, 1);
  assert.equal(app.runs[0].action, "save-best");
  assert.equal(app.$("sound").disabled, true);
  app.$("sound").dispatch("click");
  app.$("confirm-restart").dispatch("click");
  pending.resolve(app.snapshot.view);
  await settle();
  assert.deepEqual(JSON.parse(app.runs[0].fields.checkpoint), initial());
  assert.equal(app.$("save").disabled, false);
  assert.equal(app.frames.size, 0);
});

test("mouse steering records arbitrary angles and held boost ends on release", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.$("board").dispatch("pointermove", {
    pointerType: "mouse",
    isPrimary: true,
    clientX: 730,
    clientY: 370,
  });
  app.$("board").dispatch("pointerdown", {
    pointerType: "mouse",
    pointerId: 1,
    isPrimary: true,
    button: 0,
    clientX: 730,
    clientY: 370,
  });
  app.advance(51);
  app.$("board").dispatch("pointerup", {
    pointerType: "mouse",
    pointerId: 1,
    isPrimary: true,
    button: 0,
  });
  app.advance(101);
  app.click("pause");
  await settle();
  const inputs = latest(app).inputs;
  assert.ok(inputs.some(([angle]) => ![0, 64, 128, 192].includes(angle)));
  assert.ok(inputs.some(([, boost]) => boost));
  assert.equal(inputs.at(-1)[1], false);
});

test("diagonal keyboard steering and boost retain editable input behavior", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.key("d");
  app.key("s");
  app.key("Shift");
  app.advance(51);
  app.advance(101);
  app.keyup("d");
  app.keyup("s");
  app.keyup("Shift");
  app.key("a", app.$("nickname"));
  app.advance(151);
  app.click("pause");
  await settle();
  const inputs = latest(app).inputs;
  assert.ok(inputs.some(([angle]) => angle > 0 && angle < 64));
  assert.ok(inputs.some(([, boost]) => boost));
  assert.equal(inputs.at(-1)[1], false);
});

test("language and host theme updates preserve the running arena without changing status on every tick", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  const writes = app.$("notice").writes;
  app.advance(1);
  app.advance(51);
  assert.equal(app.$("notice").writes, writes);
  app.emit({ locale: "zh-CN", theme: "light" });
  assert.match(app.document.title, /Slither|贪吃蛇/i);
  assert.equal(app.$("overlay").hidden, true);
  assert.equal(app.frames.size, 1);
  app.click("pause");
  await settle();
  assert.equal(tickCount(latest(app)), 1);
});

test("menus, skins and verified records preserve an existing arena and its preferences", async (t) => {
  const app = fixture({ checkpoint: initial({ inputs: [[13, false, 2]] }) });
  t.after(() => app.dispose());
  await settle();
  app.click("menu-button");
  await settle();
  app.$("nickname").value = "自由蛇";
  app.$("nickname").dispatch("input");
  app.$("volume").value = "40";
  app.$("volume").dispatch("input");
  app.click("skins-button");
  assert.equal(app.$("skins-screen").hidden, false);
  app.click("skin-next");
  app.click("skin-select");
  await settle();
  app.click("home-button");
  assert.equal(app.$("menu-screen").hidden, false);
  app.click("scores-button");
  assert.equal(app.$("scores-screen").hidden, false);
  app.emit({ locale: "zh-CN" });
  assert.equal(app.$("scores-screen").hidden, false);
  app.click("back-scores");
  await start(app);
  app.click("pause");
  await settle();
  assert.equal(latest(app).skin, 1);
  assert.equal(latest(app).nickname, "自由蛇");
  assert.equal(latest(app).volume, 40);
  assert.equal(tickCount(latest(app)), 2);
});

test("restart confirmation traps focus and suppresses game shortcuts until dismissed", async (t) => {
  const app = fixture({ checkpoint: initial({ inputs: [[13, false, 2]] }) });
  t.after(() => app.dispose());
  await settle();
  app.click("restart");
  await settle();
  assert.equal(app.$("confirm").hidden, false);
  assert.equal(app.document.activeElement, app.$("cancel-restart"));
  app.key("Tab", app.$("cancel-restart"), { shiftKey: true });
  assert.equal(app.document.activeElement, app.$("confirm-restart"));
  app.key("Tab", app.$("confirm-restart"));
  assert.equal(app.document.activeElement, app.$("cancel-restart"));
  const writes = app.checkpoints.length;
  const sound = app.$("sound").getAttribute("aria-pressed");
  for (const key of ["m", "p", " "]) app.key(key, app.$("cancel-restart"));
  await settle();
  assert.equal(app.frames.size, 0);
  assert.equal(app.audios.length, 0);
  assert.equal(app.checkpoints.length, writes);
  assert.equal(app.$("sound").getAttribute("aria-pressed"), sound);
  app.key("Escape", app.$("cancel-restart"));
  assert.equal(app.$("confirm").hidden, true);
  assert.equal(app.document.activeElement, app.$("restart"));
});

for (const reason of ["active", "hidden", "blur", "offscreen"]) {
  test(`${reason} pauses simulation and sound, and recovery still needs a gesture`, async (t) => {
    const app = fixture();
    t.after(() => app.dispose());
    await start(app);
    app.advance(1);
    app.advance(51);
    const audio = app.audios[0];
    if (reason === "active") app.emit({ active: false });
    if (reason === "hidden") {
      app.document.hidden = true;
      app.document.dispatch("visibilitychange");
    }
    if (reason === "blur") app.window.dispatch("blur");
    if (reason === "offscreen") app.visible(false);
    await settle();
    assert.equal(app.frames.size, 0);
    assert.equal(audio.state, "suspended");
    assert.equal(app.$("overlay").hidden, false);
    if (reason === "active") app.emit({ active: true });
    if (reason === "hidden") {
      app.document.hidden = false;
      app.document.dispatch("visibilitychange");
    }
    if (reason === "blur") app.window.dispatch("focus");
    if (reason === "offscreen") app.visible(true);
    await settle();
    assert.equal(app.frames.size, 0);
    assert.equal(audio.state, "suspended");
    app.click("start");
    await settle();
    assert.equal(app.frames.size, 1);
    assert.equal(audio.state, "running");
  });
}

test("upstream Ogg sounds decode after a gesture and all active audio is released on unload", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.audios.length, 0);
  await start(app);
  const audio = app.audios[0];
  assert.equal(audio.state, "running");
  assert.ok(audio.decoded.length > 0);
  assert.ok(
    audio.decoded.every(
      (bytes) => Buffer.from(bytes).subarray(0, 4).toString() === "OggS",
    ),
  );
  assert.ok(
    audio.sources.some(
      (source) => source.buffer && source.started !== undefined,
    ),
  );
  app.dispose();
  assert.equal(audio.closed, true);
  assert.ok(audio.sources.every((source) => source.disconnected));
  assert.ok(audio.gains.every((gain) => gain.disconnected));
  const sources = audio.sources.length;
  app.click("sound");
  app.click("menu-start");
  await settle();
  assert.equal(audio.sources.length, sources);
  assert.equal(app.frames.size, 0);
});

test("late audio resume or decode cannot start a sound after disposal", async () => {
  for (const phase of ["resume", "decode"]) {
    const pending = deferred();
    const app = fixture({
      [phase === "resume" ? "onAudioResume" : "onAudioDecode"]: () =>
        pending.promise,
    });
    await start(app);
    const audio = app.audios[0];
    assert.equal(audio.sources.length, 0);
    app.dispose();
    pending.resolve();
    await settle();
    assert.equal(audio.closed, true);
    assert.equal(audio.sources.length, 0);
    assert.equal(app.frames.size, 0);
  }
});

test("late audio resume cannot reactivate an inactive arena", async (t) => {
  const pending = deferred();
  const app = fixture({ onAudioResume: () => pending.promise });
  t.after(() => app.dispose());
  await start(app);
  const audio = app.audios[0];
  app.emit({ active: false });
  assert.equal(app.frames.size, 0);
  pending.resolve();
  await settle();
  assert.equal(audio.state, "suspended");
  assert.equal(audio.sources.length, 0);
  app.emit({ active: true });
  await settle();
  assert.equal(app.frames.size, 0);
  assert.equal(audio.state, "suspended");
});

test("secondary touch pointers cannot replace steering and the independent boost button releases on cancellation", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  const board = app.$("board");
  board.dispatch("pointerdown", {
    pointerType: "touch",
    pointerId: 1,
    isPrimary: true,
    button: 0,
    clientX: 100,
    clientY: 100,
  });
  board.dispatch("pointerdown", {
    pointerType: "touch",
    pointerId: 2,
    isPrimary: false,
    button: 0,
    clientX: 400,
    clientY: 400,
  });
  board.dispatch("pointermove", {
    pointerType: "touch",
    pointerId: 1,
    clientX: 250,
    clientY: 170,
  });
  app.$("boost").dispatch("pointerdown", {
    pointerType: "touch",
    pointerId: 3,
    isPrimary: false,
    button: 0,
  });
  app.advance(51);
  board.dispatch("pointermove", {
    pointerType: "touch",
    pointerId: 2,
    clientX: 50,
    clientY: 500,
  });
  app.advance(101);
  board.dispatch("pointerup", { pointerType: "touch", pointerId: 1 });
  app.$("boost").dispatch("pointercancel", { pointerId: 3 });
  app.advance(151);
  app.click("pause");
  await settle();
  const inputs = latest(app).inputs;
  assert.equal(new Set(inputs.map(([angle]) => angle)).size, 1);
  assert.ok(![0, 64, 128, 192].includes(inputs[0][0]));
  assert.equal(inputs[0][1], true);
  assert.equal(inputs[0][2], 2);
  assert.equal(inputs.at(-1)[1], false);
});

test("spectating advances a separate arena and never saves spectator inputs into the player's run", async (t) => {
  const checkpoint = initial({
    inputs: [[13, false, 2]],
    skin: 6,
    nickname: "Pip",
  });
  const app = fixture({ checkpoint });
  t.after(() => app.dispose());
  await settle();
  app.click("menu-button");
  await settle();
  app.click("spectate");
  await settle();
  assert.equal(app.$("spectate-label").hidden, false);
  assert.equal(app.$("save").disabled, true);
  for (const now of [1, 51, 101, 151, 201]) app.advance(now);
  app.click("next-snake");
  app.click("exit-spectate");
  await settle();
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.frames.size, 0);
  assert.deepEqual(latest(app), checkpoint);
  assert.equal(app.runs.length, 0);
  await start(app);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app), checkpoint);
});

test("record tables cap at Top 10 and render imported cell content literally", async (t) => {
  const rows = Array.from({ length: 11 }, (_, index) => [
    String(100 - index),
    String(index),
    "12",
    "100",
    index === 0 ? "<img src=x onerror=alert(1)>" : "normal",
    String(index),
  ]);
  const app = fixture({
    view: {
      blocks: [
        { kind: "table", rows: [["normal", "100"]] },
        { kind: "table", rows },
      ],
    },
  });
  t.after(() => app.dispose());
  await settle();
  app.click("scores-button");
  assert.equal(app.$("leaderboard").children.length, 10);
  const cells = app.$("leaderboard").children[0].children;
  assert.equal(cells[0].textContent, "01");
  assert.equal(cells[1].textContent, "100");
  assert.equal(cells[2].textContent, "0");
  assert.equal(cells[3].textContent, "<img src=x onerror=alert(1)>");
  assert.ok(cells.every((cell) => cell.children.length === 0));
  app.emit({ locale: "zh-CN" });
  assert.equal(
    app.$("leaderboard").children[1].children[3].textContent,
    "标准",
  );
});

test("long checkpoint recovery yields and disposal prevents its late continuation from changing the page", async () => {
  const source = fixture();
  const checkpoint = initial();
  const world = source.engine.create(checkpoint.seed, checkpoint.difficulty);
  for (let tick = 0; tick < 480; tick++) {
    const input = { angle: (tick * 3) & 255, boost: false };
    source.engine.step(world, input);
    source.engine.appendInput(checkpoint, input);
  }
  source.dispose();
  const yields = [];
  const app = fixture({
    checkpoint,
    onReplayYield: (resume) => yields.push(resume),
  });
  assert.equal(yields.length, 1);
  assert.equal(app.frames.size, 0);
  assert.equal(app.checkpoints.length, 0);
  app.dispose();
  const writes = app.writes;
  yields.shift()();
  await settle();
  assert.equal(app.writes, writes);
  assert.equal(app.frames.size, 0);
  assert.equal(app.checkpoints.length, 0);
  assert.equal(app.runs.length, 0);
  assert.equal(app.audios.length, 0);
});

const beforeBoundaryDeath = () => initial({ inputs: [[0, false, 425]] });
const cueBytes = (app, name) =>
  Buffer.from(app.assets.sounds[name].split(",")[1], "base64");
const playedCue = (audio, bytes) =>
  audio.sources.find(
    (source) => source.buffer && Buffer.from(source.buffer.data).equals(bytes),
  );

test("the first uncached death cue plays after the arena pauses, then its 600 ms tail suspends and releases audio", async (t) => {
  const app = fixture({
    checkpoint: beforeBoundaryDeath(),
    onReplayYield: (resume) => queueMicrotask(resume),
    fakeAudioTimers: true,
  });
  t.after(() => app.dispose());
  await start(app);
  const audio = app.audios[0];
  const deathSound = cueBytes(app, "error_1");
  assert.equal(playedCue(audio, deathSound), undefined);
  app.advance(1);
  app.advance(51);
  assert.equal(app.frames.size, 0);
  assert.equal(app.$("overlay").hidden, false);
  await settle();
  const source = playedCue(audio, deathSound);
  assert.ok(source);
  assert.notEqual(source.started, undefined);
  assert.equal(audio.state, "running");
  assert.equal(app.audioTimers.size, 1);
  assert.equal([...app.audioTimers.keys()][0].delay, 600);
  assert.equal(tickCount(latest(app)), 426);
  app.finishAudioTail();
  assert.equal(audio.state, "suspended");
  assert.equal(source.stopped, 0);
  assert.equal(source.disconnected, true);
  assert.ok(audio.gains.every((gain) => gain.disconnected));
});

for (const interruption of ["p", "hidden"]) {
  test(`${interruption} stops a death cue whose first decode arrives after the pause`, async (t) => {
    const decoded = deferred();
    let deathSound;
    const app = fixture({
      checkpoint: beforeBoundaryDeath(),
      onReplayYield: (resume) => queueMicrotask(resume),
      fakeAudioTimers: true,
      onAudioDecode: (bytes) => {
        if (deathSound && Buffer.from(bytes).equals(deathSound))
          return decoded.promise;
      },
    });
    t.after(() => app.dispose());
    deathSound = cueBytes(app, "error_1");
    await start(app);
    const audio = app.audios[0];
    app.advance(1);
    app.advance(51);
    await settle();
    assert.ok(
      audio.decoded.some((bytes) => Buffer.from(bytes).equals(deathSound)),
    );
    assert.equal(playedCue(audio, deathSound), undefined);
    if (interruption === "p") app.key("p");
    else {
      app.document.hidden = true;
      app.document.dispatch("visibilitychange");
    }
    assert.equal(audio.state, "suspended");
    assert.equal(app.audioTimers.size, 0);
    decoded.resolve();
    await settle();
    assert.equal(playedCue(audio, deathSound), undefined);
    assert.equal(audio.state, "suspended");
    assert.equal(app.frames.size, 0);
  });
}

test("spectating a restored unsigned seed with its high bit set creates a valid independent arena", async (t) => {
  const checkpoint = initial({ seed: 4294967295 });
  const app = fixture({ checkpoint });
  t.after(() => app.dispose());
  await settle();
  assert.doesNotThrow(() => app.click("spectate"));
  assert.equal(app.$("spectate-label").hidden, false);
  app.advance(1);
  assert.doesNotThrow(() => app.advance(51));
  assert.equal(app.frames.size, 1);
  app.click("exit-spectate");
  await settle();
  assert.equal(app.$("data-error").hidden, true);
  assert.deepEqual(latest(app), checkpoint);
  assert.equal(app.runs.length, 0);
});

test("cancelling a new round after a finished arena returns focus to the visible menu start button", async (t) => {
  const app = fixture({
    checkpoint: beforeBoundaryDeath(),
    onReplayYield: (resume) => queueMicrotask(resume),
    fakeAudioTimers: true,
  });
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.advance(51);
  await settle();
  assert.equal(app.$("results").hidden, false);
  assert.equal(app.frames.size, 0);
  app.key("Escape");
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.$("game-screen").hidden, true);
  app.click("menu-start");
  assert.equal(app.$("confirm").hidden, false);
  assert.equal(app.document.activeElement, app.$("cancel-restart"));
  app.click("cancel-restart");
  await settle();
  assert.equal(app.$("confirm").hidden, true);
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.$("game-screen").hidden, true);
  assert.equal(app.document.activeElement, app.$("menu-start"));
  assert.equal(tickCount(latest(app)), 426);
  assert.equal(app.frames.size, 0);
});

test("a new gesture queues audio resume even while the previous suspend still reports running", async (t) => {
  const suspended = deferred();
  const app = fixture({
    onAudioSuspend: (_audio, count) =>
      count === 1 ? suspended.promise : undefined,
    onAudioResume: (_audio, count) =>
      count === 2 ? suspended.promise.then(() => {}) : undefined,
  });
  t.after(() => app.dispose());
  await start(app);
  const audio = app.audios[0];
  assert.equal(audio.resumptions, 1);
  const played = audio.sources.length;
  assert.ok(played > 0);
  app.click("pause");
  assert.equal(audio.state, "running");
  app.click("start");
  assert.equal(audio.resumptions, 2);
  await settle();
  assert.equal(audio.sources.length, played);
  suspended.resolve();
  await settle();
  assert.equal(audio.state, "running");
  assert.equal(audio.sources.length, played + 1);
  assert.notEqual(audio.sources.at(-1).started, undefined);
  assert.equal(app.frames.size, 1);
});

for (const firstReply of ["old", "new"]) {
  test(`a stale audio resume cannot suspend a newer gesture when the ${firstReply} reply completes first`, async (t) => {
    const oldResume = deferred();
    const newResume = deferred();
    const app = fixture({
      onAudioResume: (_audio, count) =>
        count === 1 ? oldResume.promise : newResume.promise,
    });
    t.after(() => app.dispose());
    await start(app);
    const audio = app.audios[0];
    assert.equal(audio.sources.length, 0);
    app.click("pause");
    const suspensions = audio.suspensions;
    app.click("start");
    assert.equal(audio.resumptions, 2);
    if (firstReply === "old") {
      oldResume.resolve();
      await settle();
      assert.equal(audio.suspensions, suspensions);
      assert.equal(audio.sources.length, 0);
      newResume.resolve();
    } else {
      newResume.resolve();
      await settle();
      assert.equal(audio.sources.length, 1);
      oldResume.resolve();
    }
    await settle();
    assert.equal(audio.suspensions, suspensions);
    assert.equal(audio.state, "running");
    assert.equal(audio.sources.length, 1);
    assert.notEqual(audio.sources[0].started, undefined);
    assert.equal(app.frames.size, 1);
  });
}

test("a late audio resume after user pause suspends itself when no newer gesture exists", async (t) => {
  const resumed = deferred();
  const app = fixture({ onAudioResume: () => resumed.promise });
  t.after(() => app.dispose());
  await start(app);
  const audio = app.audios[0];
  app.key("p");
  assert.equal(app.frames.size, 0);
  const suspensions = audio.suspensions;
  resumed.resolve();
  await settle();
  assert.ok(audio.suspensions > suspensions);
  assert.equal(audio.state, "suspended");
  assert.equal(audio.sources.length, 0);
  assert.equal(app.frames.size, 0);
});

test("the followed player shows its custom nickname above the head at a fixed screen font size", async (t) => {
  const app = fixture({ checkpoint: initial({ nickname: "小蛇 Alex" }) });
  t.after(() => app.dispose());
  await start(app);
  app.drawnText.length = 0;
  app.advance(1);
  const label = app.drawnText.find((entry) => entry.text === "小蛇 Alex");
  assert.ok(label, "the controlled snake must not be omitted from name labels");
  assert.equal(label.textAlign, "center");
  assert.equal(label.textBaseline, "middle");
  assert.equal(label.font, "700 13px Inter, sans-serif");
  assert.equal(label.x, app.$("board").clientWidth / 2);
  assert.ok(label.y < app.$("board").clientHeight / 2);
  assert.ok(label.y >= 18);
  app.$("board").clientWidth = 360;
  app.$("board").clientHeight = 640;
  app.drawnText.length = 0;
  app.advance(51);
  const narrowLabel = app.drawnText.find((entry) => entry.text === "小蛇 Alex");
  assert.ok(narrowLabel);
  assert.equal(narrowLabel.font, label.font);
  assert.ok(narrowLabel.x >= 8 && narrowLabel.x <= 352);
  assert.ok(narrowLabel.y >= 18 && narrowLabel.y <= 622);
});

test("an unnamed player uses the current locale while reduced motion keeps the name visible", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.emit({ locale: "zh-CN", theme: { motion: "reduced" } });
  await start(app);
  app.drawnText.length = 0;
  app.advance(1);
  assert.ok(app.drawnText.some((entry) => entry.text === "你"));
  app.drawnText.length = 0;
  app.emit({ locale: "en" });
  assert.ok(app.drawnText.some((entry) => entry.text === "You"));
});

test("spectating shows the current AI target name and updates it when changing snakes", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  app.emit({ theme: { motion: "reduced" } });
  app.click("spectate");
  app.drawnText.length = 0;
  app.advance(1);
  const first = app.drawnText.find((entry) => entry.text === "Litten");
  assert.ok(
    first,
    "the camera subject must keep its AI name in spectator mode",
  );
  assert.equal(first.font, "700 13px Inter, sans-serif");
  assert.ok(Math.abs(first.x - app.$("board").clientWidth / 2) < 1);
  app.click("next-snake");
  app.drawnText.length = 0;
  app.advance(51);
  assert.ok(
    app.drawnText.some(
      (entry) => entry.text === "Mochi" && entry.font.startsWith("700 "),
    ),
  );
});

test("sound settings open a single discoverable volume and mute panel", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.$("sound-panel").hidden, true);
  assert.equal(app.$("sound-settings").getAttribute("aria-expanded"), "false");
  assert.match(app.$("sound-settings-text").textContent, /sound/i);
  assert.match(
    app.$("sound-settings").getAttribute("aria-label"),
    /sound settings/i,
  );
  assert.equal([...html.matchAll(/\bid="volume"/g)].length, 1);
  assert.equal([...html.matchAll(/\bid="sound"/g)].length, 1);
  app.click("sound-settings");
  assert.equal(app.$("sound-panel").hidden, false);
  assert.equal(app.$("sound-panel").getAttribute("role"), "dialog");
  assert.equal(app.$("sound-settings").getAttribute("aria-expanded"), "true");
  assert.match(app.$("volume-value").textContent, /65/);
  assert.equal(app.frames.size, 0);
  app.click("sound-close");
  assert.equal(app.$("sound-panel").hidden, true);
  assert.equal(app.$("sound-settings").getAttribute("aria-expanded"), "false");
  assert.equal(app.document.activeElement, app.$("sound-settings"));
});

test("the unified sound panel updates the displayed volume and retains mute independently", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  app.click("sound-settings");
  app.$("volume").value = "31";
  app.$("volume").dispatch("input");
  app.$("volume").dispatch("change");
  await settle();
  assert.match(app.$("volume-value").textContent, /31/);
  assert.equal(latest(app).volume, 31);
  assert.equal(latest(app).muted, false);
  app.click("sound");
  await settle();
  assert.equal(latest(app).muted, true);
  assert.equal(latest(app).volume, 31);
  assert.equal(app.$("sound").getAttribute("aria-pressed"), "true");
  app.$("volume").value = "48";
  app.$("volume").dispatch("input");
  app.$("volume").dispatch("change");
  await settle();
  assert.match(app.$("volume-value").textContent, /48/);
  assert.equal(latest(app).muted, true);
  app.click("sound-close");
  app.click("sound-settings");
  assert.equal(app.$("volume").value, "48");
  assert.equal(app.$("sound").getAttribute("aria-pressed"), "true");
  app.click("sound");
  await settle();
  assert.equal(latest(app).muted, false);
  assert.equal(latest(app).volume, 48);
});

for (const dismiss of ["escape", "outside"]) {
  test(`${dismiss} dismisses sound settings and returns focus without starting the game`, async (t) => {
    const app = fixture();
    t.after(() => app.dispose());
    await settle();
    app.click("sound-settings");
    if (dismiss === "escape") app.key("Escape", app.$("volume"));
    else app.document.dispatch("pointerdown", { target: app.$("title") });
    assert.equal(app.$("sound-panel").hidden, true);
    assert.equal(
      app.$("sound-settings").getAttribute("aria-expanded"),
      "false",
    );
    assert.equal(app.document.activeElement, app.$("sound-settings"));
    assert.equal(app.$("menu-screen").hidden, false);
    assert.equal(app.$("confirm").hidden, true);
    assert.equal(app.frames.size, 0);
  });
}

test("changing menu pages closes sound settings and hides home attribution on the skin page", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.$("home-footer").hidden, false);
  app.click("sound-settings");
  app.click("skins-button");
  assert.equal(app.$("skins-screen").hidden, false);
  assert.equal(app.$("sound-panel").hidden, true);
  assert.equal(app.$("sound-settings").getAttribute("aria-expanded"), "false");
  assert.equal(app.$("home-footer").hidden, true);
  app.click("home-button");
  assert.equal(app.$("home-footer").hidden, false);
});

test("the pause overlay exposes a main menu action that preserves and resumes the same arena", async (t) => {
  const checkpoint = initial({
    inputs: [[13, false, 2]],
    nickname: "Returning snake",
    skin: 6,
  });
  const app = fixture({ checkpoint });
  t.after(() => app.dispose());
  await start(app);
  app.click("pause");
  await settle();
  assert.equal(app.$("overlay").hidden, false);
  assert.equal(app.$("restart").hidden, false);
  assert.match(app.$("overlay-menu").textContent, /main menu/i);
  assert.equal(app.$("overlay-menu").disabled, false);
  app.click("overlay-menu");
  await settle();
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.$("game-screen").hidden, true);
  assert.equal(app.frames.size, 0);
  assert.deepEqual(latest(app), checkpoint);
  assert.equal(app.runs.length, 0);
  await start(app);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app), checkpoint);
});

test("the result overlay offers one new round action and returns to the menu without discarding the score", async (t) => {
  const app = fixture({
    checkpoint: beforeBoundaryDeath(),
    onReplayYield: (resume) => queueMicrotask(resume),
    fakeAudioTimers: true,
  });
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.advance(51);
  await settle();
  const completed = structuredClone(latest(app));
  assert.equal(tickCount(completed), 426);
  assert.equal(app.$("results").hidden, false);
  assert.equal(app.$("restart").hidden, true);
  assert.equal(app.$("start").hidden, false);
  assert.equal(app.$("overlay-menu").hidden, false);
  assert.equal(app.$("overlay-menu").disabled, false);
  app.click("overlay-menu");
  await settle();
  assert.equal(app.$("menu-screen").hidden, false);
  assert.equal(app.$("game-screen").hidden, true);
  assert.equal(app.frames.size, 0);
  assert.deepEqual(latest(app), completed);
  assert.equal(app.$("confirm").hidden, true);
  assert.equal(app.runs.length, 0);
  app.click("menu-start");
  assert.equal(app.$("confirm").hidden, false);
  app.click("cancel-restart");
  assert.deepEqual(latest(app), completed);
  assert.equal(app.document.activeElement, app.$("menu-start"));
});

test("the spectate action has visible localized text instead of an unexplained circle", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  assert.match(app.$("spectate-text").textContent, /spectate/i);
  assert.match(app.$("spectate").getAttribute("aria-label"), /spectate/i);
  app.emit({ locale: "zh-CN" });
  assert.match(app.$("spectate-text").textContent, /观战/);
  assert.match(app.$("spectate").getAttribute("aria-label"), /观战/);
  app.click("spectate");
  assert.equal(app.$("spectate-label").hidden, false);
  assert.equal(app.$("game-screen").hidden, false);
  assert.equal(app.$("save").disabled, true);
});

for (const first of ["joystick", "boost"]) {
  test(`two touch controls work when ${first} owns the primary pointer`, async (t) => {
    const app = fixture();
    t.after(() => app.dispose());
    await start(app);
    app.advance(1);
    const steer = () =>
      app.$("joystick").dispatch("pointerdown", {
        pointerType: "touch",
        pointerId: 1,
        isPrimary: first === "joystick",
        button: 0,
        clientX: 580,
        clientY: 400,
      });
    const boost = () =>
      app.$("boost").dispatch("pointerdown", {
        pointerType: "touch",
        pointerId: 3,
        isPrimary: first === "boost",
        button: 0,
      });
    if (first === "joystick") {
      steer();
      boost();
    } else {
      boost();
      steer();
    }
    app.advance(51);
    app.$("boost").dispatch("pointerdown", {
      pointerType: "touch",
      pointerId: 4,
      isPrimary: false,
      button: 0,
    });
    app.$("boost").dispatch("pointercancel", { pointerId: 4 });
    app.advance(101);
    app.$("boost").dispatch("pointercancel", { pointerId: 3 });
    app.advance(151);
    app.click("pause");
    await settle();
    assert.deepEqual(latest(app).inputs, [
      [32, true, 2],
      [32, false, 1],
    ]);
    app.click("start");
    app.$("boost").dispatch("pointerdown", {
      pointerType: "touch",
      pointerId: 5,
      isPrimary: false,
      button: 0,
    });
    app.advance(201);
    app.advance(251);
    app.click("pause");
    await settle();
    assert.equal(
      latest(app).inputs.at(-1)[1],
      true,
      "pausing must release the previous touch control ownership",
    );
  });
}

test("a denied Gamepad API leaves keyboard controls and animation running", async (t) => {
  let requests = 0;
  const app = fixture({
    onGamepads() {
      requests++;
      const error = new Error("Gamepad access is denied by policy");
      error.name = "SecurityError";
      throw error;
    },
  });
  t.after(() => app.dispose());
  await start(app);
  app.key("w");
  app.key("a");
  app.advance(1);
  for (const now of [51, 101, 151]) assert.doesNotThrow(() => app.advance(now));
  assert.equal(app.frames.size, 1);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app).inputs, [[160, false, 3]]);
  assert.equal(requests, 1, "a denied API should not be called on every tick");
});

test("gamepad steering and boost release cleanly when the pad disconnects", async (t) => {
  let connected = true;
  const app = fixture({
    onGamepads: () =>
      connected
        ? [
            {
              connected: true,
              axes: [0.7, -0.7],
              buttons: [{ pressed: true }],
            },
          ]
        : [],
  });
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.advance(51);
  connected = false;
  app.key("w");
  app.key("a");
  app.advance(101);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app).inputs, [
    [224, true, 1],
    [160, false, 1],
  ]);
});

test("button sounds also play when a nested icon or text receives the pointer", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  const button = app.$("sound-settings");
  const child = app.document.createElement("span");
  button.append(child);
  app.document.dispatch("pointerdown", { target: child });
  await settle();
  app.document.dispatch("pointerup", { target: child });
  await settle();
  assert.equal(app.audios[0].sources.length, 2);
  assert.deepEqual(
    app.audios[0].sources.map((source) => source.buffer.bytes),
    [5903, 5902],
  );
  button.disabled = true;
  app.document.dispatch("pointerdown", { target: child });
  app.document.dispatch("pointerup", { target: child });
  await settle();
  assert.equal(app.audios[0].sources.length, 2);
});

test("nickname input accepts 24 Unicode characters without splitting emoji", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await settle();
  const nickname = "🙂".repeat(24);
  assert.match(html, /id="nickname"[\s\S]*?maxlength="48"/);
  app.$("nickname").value = nickname;
  app.$("nickname").dispatch("input");
  await start(app);
  app.click("pause");
  await settle();
  assert.equal(latest(app).nickname, nickname);
  assert.equal(app.$("nickname").value, nickname);
  assert.doesNotThrow(() => app.engine.validateCheckpoint(latest(app)));
});

test("nickname normalization waits for IME commit and removes invalid Unicode", async (t) => {
  const app = fixture({ checkpoint: initial({ nickname: "原名" }) });
  t.after(() => app.dispose());
  await settle();
  app.$("nickname").value = "中文\ud800";
  app.$("nickname").dispatch("input", { isComposing: true });
  app.click("help-button");
  await settle();
  assert.equal(latest(app).nickname, "原名");
  assert.equal(app.$("nickname").value, "中文\ud800");
  app.click("home-button");
  app.$("nickname").value = "中文🙂\ud800\u0000\udc00";
  app.$("nickname").dispatch("compositionend");
  assert.equal(app.$("nickname").value, "中文🙂");
  await start(app);
  app.click("pause");
  await settle();
  assert.equal(latest(app).nickname, "中文🙂");
});

test("an imported nickname with an unpaired surrogate preserves the original checkpoint", async (t) => {
  const checkpoint = initial({ nickname: "bad\ud800name" });
  const app = fixture({ checkpoint });
  t.after(() => app.dispose());
  await settle();
  assert.equal(app.$("data-error").hidden, false);
  assert.equal(app.checkpoints.length, 0);
  assert.deepEqual(app.snapshot.checkpoint, checkpoint);
  app.click("start");
  assert.equal(app.$("confirm").hidden, false);
  app.click("cancel-restart");
  assert.equal(app.checkpoints.length, 0);
  assert.deepEqual(app.snapshot.checkpoint, checkpoint);
});

for (const resumeControl of ["pause", "p", "start"]) {
  test(`${resumeControl} resumes the same spectator arena and followed snake`, async (t) => {
    const checkpoint = initial({
      nickname: "Player",
      inputs: [[13, false, 2]],
    });
    const app = fixture({ checkpoint });
    t.after(() => app.dispose());
    await settle();
    app.click("menu-button");
    const create = app.engine.create;
    const arenas = [];
    app.engine.create = (...args) => {
      const arena = create(...args);
      arenas.push(arena);
      return arena;
    };
    app.click("spectate");
    app.click("next-snake");
    app.advance(1);
    app.advance(51);
    app.advance(101);
    assert.equal(arenas.length, 1);
    const arena = arenas[0];
    assert.equal(arena.tick, 2);
    app.click("pause");
    assert.equal(app.frames.size, 0);
    assert.equal(app.$("overlay").hidden, false);
    if (resumeControl === "p") app.key("p");
    else app.click(resumeControl);
    assert.equal(arenas.length, 1);
    assert.equal(app.$("spectate-label").hidden, false);
    assert.match(app.$("spectate-name").textContent, /Mochi/);
    assert.equal(app.$("save").disabled, true);
    app.advance(151);
    app.advance(201);
    assert.equal(arena.tick, 3);
    app.click("exit-spectate");
    await settle();
    assert.deepEqual(latest(app), checkpoint);
  });
}

test("a spectator arena survives its virtual player's death without resetting the AI match", async (t) => {
  const checkpoint = initial({ seed: 123456 });
  const app = fixture({ checkpoint });
  t.after(() => app.dispose());
  await settle();
  const create = app.engine.create;
  const arenas = [];
  app.engine.create = (...args) => {
    const arena = create(...args);
    arenas.push(arena);
    return arena;
  };
  app.click("spectate");
  for (let tick = 0; tick <= 374; tick++) app.advance(1 + tick * 50);
  assert.equal(arenas.length, 1);
  assert.equal(arenas[0].tick, 374);
  assert.equal(arenas[0].snakes[0].alive, false);
  assert.ok(arenas[0].snakes.slice(1).some((snake) => snake.alive));
  app.click("pause");
  await settle();
  assert.equal(app.$("spectate-label").hidden, false);
  assert.deepEqual(latest(app), checkpoint);
});

test("spectating a completed player run still offers to continue spectating", async (t) => {
  const checkpoint = initial({
    seed: 9,
    difficulty: "easy",
    inputs: [[0, false, 426]],
  });
  const app = fixture({
    checkpoint,
    onReplayYield: (resume) => queueMicrotask(resume),
  });
  t.after(() => app.dispose());
  await settle();
  await settle();
  app.click("overlay-menu");
  app.click("spectate");
  app.click("pause");
  assert.match(app.$("start").textContent, /resume/i);
  assert.equal(app.$("results").hidden, true);
  app.click("start");
  assert.equal(app.$("confirm").hidden, true);
  assert.equal(app.$("spectate-label").hidden, false);
  assert.equal(app.frames.size, 1);
});

test("releasing keyboard steering keeps the last chosen direction", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  await start(app);
  app.key("w");
  app.advance(1);
  app.advance(51);
  app.keyup("w");
  app.advance(101);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app).inputs, [[192, false, 2]]);
});

test("centering a gamepad stick keeps its heading and releases boost", async (t) => {
  let moving = true;
  const app = fixture({
    onGamepads: () => [
      {
        connected: true,
        axes: moving ? [0, -1] : [0, 0],
        buttons: [{ pressed: moving }],
      },
    ],
  });
  t.after(() => app.dispose());
  await start(app);
  app.advance(1);
  app.advance(51);
  moving = false;
  app.advance(101);
  app.click("pause");
  await settle();
  assert.deepEqual(latest(app).inputs, [
    [192, true, 1],
    [192, false, 1],
  ]);
});

test("pausing focuses a named dialog and Tab cannot reach the covered arena", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.document.hasFocus = () => true;
  await start(app);
  assert.equal(app.$("board").inert, false);
  app.key("p");
  assert.equal(app.document.activeElement, app.$("start"));
  assert.equal(app.$("board").inert, true);
  assert.equal(app.$("overlay").getAttribute("role"), "dialog");
  assert.equal(app.$("overlay").getAttribute("aria-modal"), "true");
  app.$("overlay-menu").focus();
  app.key("Tab", app.$("overlay-menu"));
  assert.equal(app.document.activeElement, app.$("start"));
  app.key("Tab", app.$("start"), { shiftKey: true });
  assert.equal(app.document.activeElement, app.$("overlay-menu"));
  app.click("start");
  assert.equal(app.$("board").inert, false);
  assert.equal(app.document.activeElement, app.$("board"));
});

test("restart confirmation skips its disabled confirm action during a host operation", async (t) => {
  const app = fixture({ checkpoint: initial({ inputs: [[0, false, 2]] }) });
  t.after(() => app.dispose());
  await settle();
  app.click("restart");
  app.emit({ busy: true });
  assert.equal(app.$("confirm-restart").disabled, true);
  app.$("cancel-restart").focus();
  app.key("Tab", app.$("cancel-restart"));
  assert.equal(app.document.activeElement, app.$("cancel-restart"));
  app.key("Tab", app.$("cancel-restart"), { shiftKey: true });
  assert.equal(app.document.activeElement, app.$("cancel-restart"));
});

test("short arenas retain the actual canvas aspect ratio", async (t) => {
  const app = fixture();
  t.after(() => app.dispose());
  app.$("board").clientWidth = 600;
  app.$("board").clientHeight = 300;
  await start(app);
  assert.equal(app.$("board").width, 600);
  assert.equal(app.$("board").height, 300);
});
