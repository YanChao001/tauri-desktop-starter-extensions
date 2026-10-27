import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const html = readFileSync(new URL("./main.html", import.meta.url), "utf8");
const source = html.match(/<script>([\s\S]*?)<\/script>/)[1];

// Run the shipped controller without npm dependencies. Browser checks cover
// layout; Rust checks cover math. SDK results here are explicit test receipts.
function workspace(options = {}) {
  const nodes = [],
    listeners = new Map(),
    elements = new Map();
  const timers = new Map(),
    resizeObservers = [],
    windowListeners = new Map();
  let timerSerial = 0;
  const document = { activeElement: null };
  const dataKey = (name) =>
    name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
  class Element {
    constructor(tagName = "div") {
      Object.assign(this, {
        tagName: tagName.toUpperCase(),
        dataset: {},
        style: {},
        children: [],
        value: "",
        textContent: "",
        className: "",
        hidden: false,
        attributes: new Map(),
        listeners: new Map(),
      });
      this.classList = {
        contains: (name) => this.className.split(/\s+/).includes(name),
      };
      nodes.push(this);
    }
    setAttribute(key, value) {
      this.attributes.set(key, String(value));
      if (key === "id") this.id = String(value);
      if (key === "class") this.className = String(value);
      if (key === "hidden") this.hidden = true;
      if (key.startsWith("data-")) this.dataset[dataKey(key)] = String(value);
    }
    getAttribute(key) {
      if (key === "id") return this.id ?? null;
      if (key === "class") return this.className;
      if (key.startsWith("data-")) return this.dataset[dataKey(key)] ?? null;
      return this.attributes.get(key) ?? null;
    }
    removeAttribute(key) {
      this.attributes.delete(key);
      if (key === "hidden") this.hidden = false;
    }
    append(...children) {
      for (const child of children) child.parentElement = this;
      this.children.push(...children);
    }
    replaceChildren(...children) {
      for (const child of this.children) child.parentElement = null;
      this.children = [];
      this.append(...children);
    }
    addEventListener(name, callback) {
      const callbacks = this.listeners.get(name) ?? [];
      callbacks.push(callback);
      this.listeners.set(name, callbacks);
    }
    focus() {
      document.activeElement = this;
      this.onfocus?.({ target: this });
    }
    click() {
      if (!this.disabled) dispatch(this, "click");
    }
    contains(node) {
      for (; node; node = node.parentElement) if (node === this) return true;
      return false;
    }
    matches(selector) {
      if (selector.startsWith(":")) return false;
      const tag = selector.match(/^[a-z]+/i)?.[0];
      if (tag && this.tagName !== tag.toUpperCase()) return false;
      const id = selector.match(/#([\w-]+)/)?.[1];
      if (id && this.id !== id) return false;
      for (const [, name] of selector.matchAll(/\.([\w-]+)/g))
        if (!this.classList.contains(name)) return false;
      for (const [, name, value] of selector.matchAll(
        /\[([\w-]+)(?:=["']?([^\]"']+)["']?)?\]/g,
      )) {
        const actual = this.getAttribute(name);
        if (actual === null || (value !== undefined && actual !== value))
          return false;
      }
      return true;
    }
    querySelectorAll(selector) {
      return this.children.flatMap((child) =>
        child instanceof Element
          ? [
              ...(selector.split(",").some((part) => child.matches(part.trim()))
                ? [child]
                : []),
              ...child.querySelectorAll(selector),
            ]
          : [],
      );
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    }
    setSelectionRange(start, end) {
      this.selectionStart = start;
      this.selectionEnd = end;
    }
    getContext() {
      return null;
    }
    get firstChild() {
      return this.children[0] ?? null;
    }
    getBoundingClientRect() {
      return {
        x: 0,
        y: 0,
        left: 0,
        top: 0,
        right: 420,
        bottom: 48,
        width: 420,
        height: 48,
      };
    }
  }
  const container = new Element("document"),
    stack = [container];
  const markup = html.replace(/<(script|style)\b[^>]*>[\s\S]*?<\/\1>/g, "");
  for (const [token, tag, attributes] of markup.matchAll(
    /<\/?([a-z][\w-]*)\b([^>]*)>/gi,
  )) {
    if (token.startsWith("</")) {
      if (stack.at(-1).tagName === tag.toUpperCase()) stack.pop();
      continue;
    }
    const node = new Element(tag);
    for (const [, key, double, single, bare] of attributes.matchAll(
      /([\w-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g,
    ))
      node.setAttribute(key, double ?? single ?? bare ?? "");
    if (node.id) elements.set(node.id, node);
    stack.at(-1).append(node);
    if (
      !/^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(
        tag,
      ) &&
      !token.endsWith("/>")
    )
      stack.push(node);
  }
  Object.assign(document, {
    documentElement: container.querySelector("html"),
    getElementById: (id) =>
      nodes.find((node) => node.id === id && container.contains(node)) ?? null,
    createElement: (tag) => new Element(tag),
    createTextNode: (text) => ({ textContent: text }),
    querySelector: (selector) => container.querySelector(selector),
    querySelectorAll: (selector) => container.querySelectorAll(selector),
    addEventListener: (name, callback) => {
      const callbacks = listeners.get(name) ?? [];
      callbacks.push(callback);
      listeners.set(name, callbacks);
    },
  });
  function dispatch(target, name, values = {}) {
    const event = {
      target,
      defaultPrevented: false,
      stopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.stopped = true;
      },
      ...values,
    };
    for (let node = target; node; node = node.parentElement) {
      node["on" + name]?.(event);
      for (const callback of node.listeners?.get(name) ?? []) callback(event);
      if (event.stopped) return event;
    }
    for (const callback of listeners.get(name) ?? []) callback(event);
    return event;
  }
  let onChange;
  const calls = [],
    responses = [];
  const fields = {
    expression: "0",
    angle: "deg",
    operand: "",
    functions: "x",
    xmin: "-10",
    xmax: "10",
    ymin: "-3",
    ymax: "3",
    a: "0",
    b: "1",
    curve: "0",
    mode: options.mode ?? "scientific",
    radix: "10",
    bits: "64",
    dateFrom: "2024-01-01",
    dateTo: "2024-01-02",
    dateYears: "0",
    dateMonths: "0",
    dateDays: "0",
    dateOperation: "difference",
    left: "",
    right: "",
    operator: "",
    memoryIndex: "",
    converterCategory: "length",
    converterFrom: "",
    converterTo: "",
    converterInput: "0",
    ...options.fields,
  };
  const initialState = {
    schema: 1,
    ans: 0,
    ans_exact: "0",
    result: "0",
    memory: 0,
    memory_set: false,
    memories: [],
    programmer: null,
    date: null,
    carry: false,
    fields,
    history: [],
    plot: { series: [], xmin: -10, xmax: 10, ymin: -3, ymax: 3 },
    ...options.state,
  };
  if (
    !options.state?.memories &&
    (initialState.memory_set || initialState.memory !== 0)
  )
    initialState.memories = [String(initialState.memory)];
  const actions = [
    "calculate",
    "evaluate",
    "standard-evaluate",
    "standard-unary",
    "standard-calculate",
    "programmer-evaluate",
    "programmer-preview",
    "programmer-clear",
    "programmer-toggle-bit",
    "date-difference",
    "date-offset",
    "converter-convert",
    "currency-refresh",
    "memory-store",
    "memory-add",
    "memory-subtract",
    "memory-clear",
    "memory-recall",
    "memory-remove",
    "history-clear",
    "plot",
    "reset-plot",
    "root",
    "integrate",
    "save",
    "load",
    "export",
  ].map((id) => ({
    id,
    disabled: options.disabledActions?.includes(id) ?? false,
  }));
  const snapshot = {
    locale: "en",
    busy: false,
    active: true,
    dirty: false,
    grantedPermissions: options.permissions ?? [],
    fields,
    checkpoint: options.checkpoint ?? {
      schema: 1,
      fields,
      calculator: { mode: options.mode ?? "scientific" },
    },
    view: {
      state: JSON.stringify(initialState),
      errors: {},
      artifacts: [],
      fields: Object.entries(fields).map(([id, value]) => ({ id, value })),
      groups: [{ actions }],
    },
  };
  let currentState = initialState;
  const starter = {
    services: {
      request: async (service, payload) => {
        calls.push({
          method: "service",
          service,
          payload: structuredClone(payload),
        });
        if (options.serviceRequest)
          return options.serviceRequest(service, payload);
        return {};
      },
    },
    onChange: (callback) => {
      onChange = callback;
    },
    notify: async (message) => {
      calls.push({ method: "notify", message });
    },
    setFields: async (value) => {
      calls.push({ method: "setFields", value: structuredClone(value) });
      if (options.setFieldsRequest) await options.setFieldsRequest(value);
    },
    checkpoint: async (value, dirty) => {
      calls.push({
        method: "checkpoint",
        value: structuredClone(value),
        dirty,
      });
    },
    run: async (action, submitted) => {
      calls.push({ method: "run", action, fields: structuredClone(submitted) });
      const response = responses.shift() ?? {};
      currentState = {
        ...currentState,
        fields: { ...submitted, operand: "" },
        ...response.state,
      };
      return {
        ...snapshot.view,
        fields: Object.entries(currentState.fields).map(([id, value]) => ({
          id,
          value,
        })),
        state: JSON.stringify(currentState),
        errors: response.errors ?? {},
      };
    },
  };
  vm.runInNewContext(source, {
    document,
    starter,
    window: {
      devicePixelRatio: 1,
      innerWidth: 420,
      innerHeight: 720,
      addEventListener(name, callback) {
        const callbacks = windowListeners.get(name) ?? [];
        callbacks.push(callback);
        windowListeners.set(name, callbacks);
      },
      getSelection: () => ({ toString: () => "" }),
      getComputedStyle: () => ({ columnGap: "24px" }),
    },
    structuredClone,
    ResizeObserver: class {
      constructor(callback) {
        this.callback = callback;
        this.targets = new Set();
        resizeObservers.push(this);
      }
      observe(target) {
        this.targets.add(target);
      }
      disconnect() {
        this.targets.clear();
      }
    },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    requestAnimationFrame: () => 1,
    cancelAnimationFrame() {},
    setTimeout: (callback, delay) => {
      const id = ++timerSerial;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout: (id) => timers.delete(id),
    performance: { now: () => 0 },
  });
  onChange(snapshot);
  if (
    options.mode &&
    document.querySelector("main").dataset.mode !== options.mode
  )
    elements.get(options.mode + "-mode").click();
  const press = (...keys) => {
    for (const key of keys) {
      const button = elements
        .get("keypad")
        .children.find((node) => node.dataset.key === key);
      assert.ok(button, `Missing calculator key: ${key}`);
      button.click();
    }
  };
  const input = (value) => {
    const node = elements.get("expression");
    node.value = value;
    node.setSelectionRange(value.length, value.length);
    dispatch(node, "input");
  };
  return {
    elements,
    document,
    snapshot,
    onChange,
    calls,
    responses,
    press,
    input,
    key: (key, target = container.querySelector("main"), modifiers = {}) =>
      dispatch(target, "keydown", { key, ...modifiers }),
    expression: () => elements.get("expression").value,
    display: () => elements.get("result").textContent,
    action: (id) => document.querySelector(`[data-action="${id}"]`).click(),
    pressProgrammer: (...keys) => {
      for (const key of keys) {
        const button = elements
          .get("prog-keypad")
          .children.find((node) => node.dataset.key === key);
        assert.ok(button, `Missing programmer key: ${key}`);
        button.click();
      }
    },
    pressConverter: (...keys) => {
      for (const key of keys) {
        const button = elements
          .get("converter-keypad")
          .querySelector(`[data-converter-key="${key}"]`);
        assert.ok(button, `Missing converter key: ${key}`);
        button.click();
      }
    },
    event: dispatch,
    windowEvent: (name) => {
      for (const callback of windowListeners.get(name) ?? []) callback();
    },
    resize: (target) => {
      const observers = resizeObservers.filter((observer) =>
        observer.targets.has(target),
      );
      assert.ok(observers.length, "Element must have an active ResizeObserver");
      for (const observer of observers) observer.callback([{ target }]);
    },
    flushTimers: async () => {
      for (let iteration = 0; iteration < 10 && timers.size; iteration++) {
        const pending = [...timers.entries()];
        for (const [id, timer] of pending) {
          if (!timers.delete(id)) continue;
          timer.callback();
          await settle();
        }
      }
    },
  };
}

async function settle() {
  for (let index = 0; index < 60; index++) await Promise.resolve();
}

function programmerOutput(overrides = {}) {
  return {
    value: "0",
    dec: "0",
    hex: "0",
    oct: "0",
    bin: "0",
    unsigned: "0",
    radix: 10,
    bits: 64,
    carry: false,
    ...overrides,
  };
}

test("analysis results replace restored input previews", async () => {
  for (const action of ["root", "integrate"]) {
    const app = workspace({
      checkpoint: {
        schema: 1,
        fields: { expression: "sin(90)" },
        calculator: {
          mode: "graph",
          displayValue: "sin(90)",
          justEvaluated: false,
        },
      },
    });
    app.responses.push({ state: { result: "2", ans: 2 } });
    app.action(action);
    await settle();
    assert.equal(app.display(), "2");
    const saved = app.calls
      .filter((call) => call.method === "checkpoint")
      .at(-1).value;
    assert.equal(saved.calculator.displayValue, null);
    assert.equal(saved.calculator.justEvaluated, true);
    assert.equal(saved.calculator.repeatOperation, null);
    assert.equal(app.elements.get("analysis-result").textContent, "2");
  }
});

test("backspace deletes a complete Unicode code point and respects selections", () => {
  const app = workspace();
  app.input("2+😀");
  app.press("backspace");
  assert.equal(app.expression(), "2+");
  assert.equal(app.elements.get("expression").selectionStart, 2);
  app.input("12+34");
  app.elements.get("expression").setSelectionRange(3, 5);
  app.press("backspace");
  assert.equal(app.expression(), "12+");
});

test("unchanged host snapshots preserve focused history elements", () => {
  const app = workspace({
    state: { history: [{ expression: "1+1", value: 2, angle: "rad" }] },
  });
  const row = app.elements.get("history").children[0].children[0];
  row.focus();
  app.onChange({ ...app.snapshot });
  assert.equal(app.elements.get("history").children[0].children[0], row);
  assert.equal(app.document.activeElement, row);
});

test("locale changes update the document language for assistive technology", () => {
  const app = workspace();
  assert.equal(app.document.documentElement.lang, "en");
  app.onChange({ ...app.snapshot, locale: "zh-CN" });
  assert.equal(app.document.documentElement.lang, "zh-CN");
});

test("digits replace the initial zero and decimal input is limited per operand", () => {
  const app = workspace();
  app.press("0", "2", "decimal", "5", "decimal", "add", "decimal", "7");
  assert.equal(app.expression(), "2.5+0.7");
  assert.equal(app.display(), "0.7");
});

test("operators replace unfinished operators without discarding prior operands", () => {
  const app = workspace();
  app.press("2", "add", "multiply", "3");
  assert.equal(app.expression(), "2*3");
  app.key("F9");
  assert.equal(app.expression(), "2*-3");
  app.key("F9");
  assert.equal(app.expression(), "2*3");
});

test("CE removes the current operand while Escape clears the full calculation", () => {
  const app = workspace();
  app.press("2", "add", "3");
  app.key("Delete");
  assert.equal(app.expression(), "2+0");
  app.press("4");
  assert.equal(app.expression(), "2+4");
  app.key("Escape");
  assert.equal(app.expression(), "0");
  assert.equal(app.display(), "0");
});

test("exp enters a signed decimal exponent without inserting an e power function", () => {
  const app = workspace();
  app.press("1", "exp", "subtract", "3");
  assert.equal(app.expression(), "1e-3");
  app.key("F9");
  assert.equal(app.expression(), "1e+3");
});

test("parentheses insert multiplication and equals closes outstanding groups", async () => {
  const app = workspace();
  app.responses.push({ state: { result: "14", ans: 14 } });
  app.press("2", "open", "3", "add", "4", "equals");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "calculate");
  assert.equal(request.fields.expression, "2*(3+4)");
  assert.equal(app.display(), "14");
});

test("unary evaluates only the current operand and retains the pending expression", async () => {
  const app = workspace();
  app.responses.push({ state: { result: "9" } });
  app.press("2", "add", "3", "square");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "evaluate");
  assert.equal(request.fields.operand, "((3)^2)");
  assert.equal(request.fields.expression, "2+((3)^2)");
  assert.equal(app.display(), "9");
  assert.equal(app.elements.get("operand").value, "");
  app.press("multiply", "4");
  assert.equal(app.expression(), "2+((3)^2)*4");
});

test("second functions switch in place and reset after use", async () => {
  const app = workspace();
  app.press("2", "second");
  assert.equal(
    app.document.getElementById("second").getAttribute("aria-pressed"),
    "true",
  );
  for (const key of ["cube", "cbrt", "root", "pow2", "logbase", "powe"])
    assert.ok(
      app.elements
        .get("keypad")
        .children.some((button) => button.dataset.key === key),
    );
  app.responses.push({ state: { result: "8" } });
  app.press("cube");
  await settle();
  assert.equal(app.display(), "8");
  assert.equal(
    app.document.getElementById("second").getAttribute("aria-pressed"),
    "false",
  );
  assert.ok(
    app.elements
      .get("keypad")
      .children.some((button) => button.dataset.key === "square"),
  );
});

test("equals repeats the final operation and a new digit starts a new calculation", async () => {
  const app = workspace();
  app.responses.push({ state: { result: "5", ans: 5 } });
  app.press("2", "add", "3", "equals");
  await settle();
  app.responses.push({ state: { result: "8", ans: 8 } });
  app.key("Enter");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields.expression,
    "(5)+(3)",
  );
  app.press("7");
  assert.equal(app.expression(), "7");
  assert.equal(app.display(), "7");
});

test("memory stores the displayed operand and recall replaces only the current operand", async () => {
  const app = workspace({ state: { memory: 12, memory_set: true } });
  app.press("2", "add", "3");
  app.responses.push({ state: { memory: 3, memories: ["3", "12"] } });
  app.action("memory-store");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "memory-store");
  assert.equal(request.fields.operand, "3");
  app.document.querySelector('[data-memory-recall="1"]').click();
  assert.equal(app.expression(), "2+12");
  assert.equal(app.display(), "12");
});

test("a stored zero enables MR and MC disables it after its receipt", async () => {
  const app = workspace({ state: { memory: 0, memory_set: true } });
  assert.equal(app.elements.get("recall").disabled, false);
  app.responses.push({ state: { memory: 0, memory_set: false, memories: [] } });
  app.action("memory-clear");
  await settle();
  assert.equal(app.elements.get("recall").disabled, true);
  assert.equal(app.elements.get("memory-card").hidden, true);
});

test("trig inverse and hyp combine and reset after selecting a function", async () => {
  const app = workspace();
  app.press("1");
  app.elements.get("trig-menu").click();
  app.elements.get("trig-second").click();
  app.elements.get("trig-hyp").click();
  const sine = app.document.querySelector('[data-trig="sin"]');
  assert.equal(sine.textContent, "sinh⁻¹");
  sine.click();
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.fields.operand, "(asinh(1))");
  assert.equal(app.elements.get("trig-popup").hidden, true);
  assert.equal(
    app.elements.get("trig-second").getAttribute("aria-pressed"),
    "false",
  );
  assert.equal(
    app.elements.get("trig-hyp").getAttribute("aria-pressed"),
    "false",
  );
});

test("angle cycle and function keys select the three angle units", () => {
  const app = workspace();
  for (const mode of ["rad", "grad", "deg"]) {
    app.elements.get("angle-cycle").click();
    assert.equal(app.elements.get("angle").value, mode);
  }
  for (const [key, mode] of [
    ["F5", "grad"],
    ["F4", "rad"],
    ["F3", "deg"],
  ]) {
    app.key(key);
    assert.equal(app.elements.get("angle").value, mode);
  }
});

test("notation changes presentation while preserving the submitted expression", () => {
  const app = workspace();
  app.input("1234");
  app.elements.get("notation").click();
  assert.equal(app.display(), "1.234e+3");
  assert.equal(app.expression(), "1234");
  app.elements.get("notation").click();
  assert.equal(app.display(), "1,234");
});

test("Escape closes menus and restores focus without erasing a calculation", () => {
  const app = workspace();
  app.press("2", "add", "3");
  app.elements.get("functions-menu").click();
  app.key("Escape");
  assert.equal(app.elements.get("functions-popup").hidden, true);
  assert.equal(app.document.activeElement, app.elements.get("functions-menu"));
  assert.equal(app.expression(), "2+3");
});

test("history and memory tabs support keyboard navigation and narrow panel close", () => {
  const app = workspace();
  app.elements.get("history-toggle").click();
  app.key("ArrowRight", app.elements.get("history-tab"));
  assert.equal(
    app.elements.get("memory-tab").getAttribute("aria-selected"),
    "true",
  );
  assert.equal(app.elements.get("history-content").hidden, true);
  assert.equal(app.elements.get("memory-content").hidden, false);
  app.key("Escape");
  assert.equal(app.document.querySelector("main").dataset.railOpen, "false");
  assert.equal(app.document.activeElement, app.elements.get("memory-toggle"));
});

test("switching history and memory updates the clear button label and enabled state", () => {
  for (const [locale, historyLabel, memoryLabel] of [
    ["en", "Clear history", "Clear memory"],
    ["zh-CN", "清空历史记录", "清空记忆"],
  ]) {
    const app = workspace({
      state: { memories: ["7"], memory: 7, memory_set: true },
    });
    app.onChange({ ...app.snapshot, locale });
    const clear = app.elements.get("clear-rail");
    assert.equal(clear.disabled, true);
    assert.equal(clear.title, historyLabel);
    assert.equal(clear.getAttribute("aria-label"), historyLabel);
    app.elements.get("memory-tab").click();
    assert.equal(clear.disabled, false);
    assert.equal(clear.title, memoryLabel);
    assert.equal(clear.getAttribute("aria-label"), memoryLabel);
    app.elements.get("history-tab").click();
    assert.equal(clear.disabled, true);
    assert.equal(clear.title, historyLabel);
  }
});

test("the rail clear button targets only its selected history or memory collection", async () => {
  for (const [tab, action] of [
    ["history", "history-clear"],
    ["memory", "memory-clear"],
  ]) {
    const app = workspace({
      state: {
        history: [
          { expression: "2+2", value: 4, angle: "deg", mode: "scientific" },
        ],
        memories: ["7"],
        memory: 7,
        memory_set: true,
      },
    });
    app.elements.get(tab + "-tab").click();
    app.elements.get("clear-rail").click();
    await settle();
    const runs = app.calls.filter((call) => call.method === "run");
    assert.equal(runs.length, 1);
    assert.equal(runs[0].action, action);
  }
});

test("the rail clear button retains permission denial after switching collections", () => {
  const app = workspace({
    state: {
      history: [
        { expression: "2+2", value: 4, angle: "deg", mode: "scientific" },
      ],
      memories: ["7"],
      memory: 7,
      memory_set: true,
    },
    disabledActions: ["history-clear", "memory-clear"],
  });
  const clear = app.elements.get("clear-rail");
  for (const tab of ["memory", "history"]) {
    app.elements.get(tab + "-tab").click();
    assert.equal(clear.disabled, true);
    assert.equal(clear.dataset.locked, "false");
    clear.click();
  }
  assert.equal(app.calls.length, 0);
});

test("busy or inactive host snapshots block calculator edits and SDK actions", () => {
  for (const change of [{ busy: true }, { active: false }]) {
    const app = workspace();
    app.onChange({ ...app.snapshot, ...change });
    app.press("2", "equals");
    app.key("9");
    app.elements.get("angle-cycle").click();
    assert.equal(app.expression(), "0");
    assert.equal(app.elements.get("angle").value, "deg");
    assert.equal(app.calls.length, 0);
  }
});

test("temporary calculator locks preserve normal visual markers while unavailable controls stay disabled", () => {
  const app = workspace({ mode: "standard" });
  const keypad = app.elements.get("keypad");
  const available = [
    keypad.querySelector('[data-key="2"]'),
    keypad.querySelector('[data-key="add"]'),
    keypad.querySelector('[data-key="equals"]'),
    app.elements.get("navigation"),
  ];
  const unavailable = [
    app.elements.get("recall"),
    app.document.querySelector('[data-action="memory-clear"]'),
    app.elements.get("clear-rail"),
    app.elements.get("always-on-top"),
  ];
  for (const change of [{ busy: true }, { active: false }]) {
    app.onChange({ ...app.snapshot, ...change });
    for (const button of available) {
      assert.equal(button.disabled, true);
      assert.equal(button.dataset.locked, "true");
    }
    for (const button of unavailable) {
      assert.equal(button.disabled, true);
      assert.equal(button.dataset.locked, "false");
    }
    app.press("2", "equals");
    app.event(available[2], "click");
    app.key("Enter");
    assert.equal(app.expression(), "0");
    assert.equal(app.calls.length, 0);
    app.onChange({ ...app.snapshot });
    for (const button of available) {
      assert.equal(button.disabled, false);
      assert.equal(button.dataset.locked, "false");
    }
    for (const button of unavailable) assert.equal(button.disabled, true);
  }
});

test("memory locks keep granted operations readable without masking denied actions", () => {
  const app = workspace({
    state: { memories: ["0"], memory: 0, memory_set: true },
    disabledActions: ["memory-clear", "memory-add"],
  });
  app.onChange({ ...app.snapshot, busy: true });
  for (const button of [
    app.elements.get("recall"),
    app.document.querySelector('[data-memory-recall="0"]'),
    app.document.querySelector('[data-memory-action="memory-remove"]'),
  ]) {
    assert.equal(button.disabled, true);
    assert.equal(button.dataset.locked, "true");
  }
  for (const button of [
    app.document.querySelector('[data-action="memory-clear"]'),
    app.document.querySelector('[data-action="memory-add"]'),
    app.document.querySelector('[data-memory-action="memory-add"]'),
  ]) {
    assert.equal(button.disabled, true);
    assert.equal(button.dataset.locked, "false");
    app.event(button, "click");
  }
  assert.equal(app.calls.length, 0);
  app.onChange({ ...app.snapshot });
  assert.equal(app.elements.get("recall").disabled, false);
  assert.equal(
    app.document.querySelector('[data-memory-action="memory-add"]').disabled,
    true,
  );
});

test("new history memory graph and artifact controls inherit an in-flight lock", () => {
  const app = workspace();
  const state = {
    ...JSON.parse(app.snapshot.view.state),
    history: [
      { expression: "2+2", value: 4, angle: "deg", mode: "scientific" },
    ],
    memory: 7,
    memory_set: true,
    memories: ["7"],
  };
  app.onChange({
    ...app.snapshot,
    busy: true,
    fields: { ...app.snapshot.fields, functions: "sin(x)\ncos(x)" },
    view: {
      ...app.snapshot.view,
      state: JSON.stringify(state),
      artifacts: [{ name: "calculation.csv" }],
    },
  });
  const buttons = [
    ...app.elements.get("history").querySelectorAll("button"),
    ...app.elements.get("memory-list").querySelectorAll("button"),
    ...app.elements.get("equations").querySelectorAll("button"),
    ...app.elements.get("artifacts").querySelectorAll("button"),
  ];
  assert.equal(app.elements.get("equations").children.length, 3);
  assert.equal(buttons.length, 15);
  for (const button of buttons) {
    assert.equal(button.disabled, true);
    assert.equal(button.dataset.locked, "true");
    button.click();
  }
  assert.equal(app.calls.length, 0);
});

test("programmer busy locks retain disabled radix digits and out-of-width bits", () => {
  const app = workspace({
    mode: "programmer",
    fields: { radix: "2", bits: "8" },
  });
  app.onChange({ ...app.snapshot, busy: true });
  for (const [key, locked] of [
    ["1", "true"],
    ["F", "false"],
    ["decimal", "false"],
  ]) {
    const button = app.elements
      .get("prog-keypad")
      .querySelector(`[data-key="${key}"]`);
    assert.equal(button.disabled, true);
    assert.equal(button.dataset.locked, locked);
  }
  for (const [bit, locked] of [
    ["7", "true"],
    ["63", "false"],
  ]) {
    const button = app.document.querySelector(`[data-bit="${bit}"]`);
    assert.equal(button.disabled, true);
    assert.equal(button.dataset.locked, locked);
  }
  app.pressProgrammer("1", "equals");
  assert.equal(app.calls.length, 0);
  app.onChange({ ...app.snapshot });
  assert.equal(
    app.elements.get("prog-keypad").querySelector('[data-key="1"]').disabled,
    false,
  );
  assert.equal(
    app.elements.get("prog-keypad").querySelector('[data-key="F"]').disabled,
    true,
  );
  assert.equal(app.document.querySelector('[data-bit="63"]').disabled, true);
});

test("calendar busy locks distinguish valid dates from supported-range boundaries", () => {
  const app = workspace({ mode: "date", fields: { dateFrom: "1601-01-01" } });
  app.elements.get("date-from-button").click();
  app.onChange({ ...app.snapshot, busy: true });
  const valid = app.document.querySelector('[data-date="1601-01-01"]');
  const outside = app.elements
    .get("calendar-grid")
    .querySelectorAll("button")
    .find((button) => button.dataset.date < "1601-01-01");
  for (const control of [
    valid,
    app.elements.get("calendar-next"),
    app.elements.get("calendar-month"),
    app.elements.get("date-add"),
  ]) {
    assert.equal(control.disabled, true);
    assert.equal(control.dataset.locked, "true");
  }
  for (const control of [outside, app.elements.get("calendar-prev")]) {
    assert.equal(control.disabled, true);
    assert.equal(control.dataset.locked, "false");
  }
  valid.click();
  assert.equal(app.elements.get("calendar-popup").hidden, false);
  assert.equal(app.calls.length, 0);
  app.onChange({ ...app.snapshot });
  assert.equal(valid.disabled, false);
  assert.equal(outside.disabled, true);
  assert.equal(app.elements.get("calendar-prev").disabled, true);
});

test("converter busy locks preserve valid values and units while unavailable rates remain disabled", () => {
  const app = converterWorkspace();
  app.onChange({ ...app.snapshot, busy: true });
  for (const control of [
    app.elements.get("converter-value-from"),
    app.elements.get("converter-value-to"),
    app.elements.get("converter-from"),
    app.elements.get("converter-to"),
    app.elements
      .get("converter-keypad")
      .querySelector('[data-converter-key="7"]'),
  ]) {
    assert.equal(control.disabled, true);
    assert.equal(control.dataset.locked, "true");
  }
  app.pressConverter("7");
  assert.equal(app.elements.get("converterInput").value, "2");
  assert.equal(app.calls.length, 0);

  const currency = converterWorkspace(
    converterOutput({ category: "currency", result: null, units: [] }),
    { disabledActions: ["currency-refresh"] },
  );
  currency.onChange({ ...currency.snapshot, busy: true });
  for (const control of [
    currency.elements.get("converter-value-to"),
    currency.elements.get("converter-from"),
    currency.elements.get("converter-to"),
    currency.elements.get("currency-refresh"),
  ]) {
    assert.equal(control.disabled, true);
    assert.equal(control.dataset.locked, "false");
  }
  assert.equal(
    currency.elements.get("converter-value-from").dataset.locked,
    "true",
  );
  currency.elements.get("currency-refresh").click();
  assert.equal(currency.calls.length, 0);
});

test("graph fields keep ordinary input and returning to scientific restores its expression", () => {
  const app = workspace();
  app.press("2", "add", "3");
  app.elements.get("graph-mode").click();
  assert.equal(app.elements.get("graph-panel").hidden, false);
  assert.equal(
    app.key("7", app.elements.get("functions")).defaultPrevented,
    false,
  );
  assert.equal(app.expression(), "0");
  app.elements.get("scientific-mode").click();
  assert.equal(app.elements.get("graph-panel").hidden, true);
  assert.equal(app.expression(), "2+3");
});

test("history recalls continue from the selected answer instead of the latest result", async () => {
  const history = [{ expression: "2+3", value: 5, angle: "deg" }];
  for (const operation of ["add", "square", "equals"]) {
    const app = workspace({ state: { result: "10", ans: 10, history } });
    app.elements.get("history").children[0].children[0].click();
    assert.equal(app.display(), "5");
    app.press(operation);
    if (operation === "add") {
      app.press("2");
      assert.equal(app.expression(), "5+2");
    } else {
      await settle();
      const request = app.calls.find((call) => call.method === "run");
      assert.equal(
        operation === "square"
          ? request.fields.operand
          : request.fields.expression,
        operation === "square" ? "((5)^2)" : "(5)+(3)",
      );
    }
  }
});

test("exp after equals continues from the displayed result", async () => {
  const app = workspace();
  app.responses.push({ state: { result: "5", ans: 5 } });
  app.press("2", "add", "3", "equals");
  await settle();
  app.press("exp", "2");
  assert.equal(app.expression(), "5e+2");
});

test("sign after a unary preview displays a numerical result", async () => {
  const app = workspace();
  app.responses.push({ state: { result: "9" } });
  app.press("3", "square");
  await settle();
  app.responses.push({ state: { result: "-9" } });
  app.press("sign");
  await settle();
  assert.equal(app.display(), "-9");
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "evaluate",
  );
});

test("unary operands include function names that contain digits", async () => {
  const app = workspace();
  app.input("2+log2(8)");
  app.press("sqrt");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.fields.operand, "(sqrt(log2(8)))");
  assert.equal(request.fields.expression, "2+(sqrt(log2(8)))");
});

test("repeat equals distinguishes Euler's constant from a decimal exponent", async () => {
  const app = workspace();
  app.input("e+2");
  app.responses.push({
    state: { result: "4.718281828459045", ans: 4.718281828459045 },
  });
  app.press("equals");
  await settle();
  app.press("equals");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields.expression,
    "(4.718281828459045)+(2)",
  );
});

test("mode and notation restore from a session checkpoint without changing the expression", () => {
  const app = workspace({
    fields: { expression: "1234" },
    state: { result: "1234" },
    checkpoint: {
      schema: 1,
      fields: { expression: "1234" },
      calculator: { mode: "graph", notation: true, justEvaluated: false },
    },
  });
  assert.equal(app.document.querySelector("main").dataset.mode, "graph");
  assert.equal(app.elements.get("graph-panel").hidden, false);
  assert.equal(
    app.elements.get("notation").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.expression(), "1234");
  assert.equal(app.display(), "1.234e+3");
});

test("actions acknowledge the latest input checkpoint before running Wasm", async () => {
  const app = workspace();
  app.press("4", "2");
  app.responses.push({ state: { result: "42", ans: 42 } });
  app.press("equals");
  await settle();
  assert.deepEqual(
    app.calls.slice(0, 3).map((call) => call.method),
    ["setFields", "checkpoint", "run"],
  );
  assert.equal(app.calls[0].value.expression, "42");
  assert.equal(app.calls[1].value.fields.expression, "42");
  assert.equal(app.calls[1].dirty, true);
  assert.equal(app.calls[2].fields.expression, "42");
});

test("completed actions retain notation and a clean operand in the recovery checkpoint", async () => {
  const app = workspace();
  app.elements.get("notation").click();
  app.responses.push({ state: { result: "9" } });
  app.press("3", "square");
  await settle();
  const saved = app.calls.filter((call) => call.method === "checkpoint").at(-1);
  assert.equal(saved.value.calculator.notation, true);
  assert.equal(saved.value.fields.operand, "");
  assert.equal(saved.value.fields.expression, "((3)^2)");
  assert.equal(saved.dirty, true);
});

test("a session import replaces stale local input and restores its own mode", () => {
  const app = workspace();
  app.press("9", "9");
  app.onChange({
    ...app.snapshot,
    fields: { ...app.snapshot.fields, expression: "1234" },
    checkpoint: {
      schema: 1,
      fields: { expression: "1234" },
      calculator: { mode: "graph", notation: true },
    },
    view: {
      ...app.snapshot.view,
      state: JSON.stringify({
        ...JSON.parse(app.snapshot.view.state),
        result: "1234",
      }),
    },
  });
  assert.equal(app.expression(), "1234");
  assert.equal(app.document.querySelector("main").dataset.mode, "graph");
  assert.equal(app.display(), "1.234e+3");
});

test("the clear key changes from CE to C and clears the full expression on its second click", () => {
  const app = workspace();
  const clear = app.elements
    .get("keypad")
    .children.find((button) => button.dataset.key === "clear");
  app.press("2", "add", "3");
  assert.equal(clear.textContent, "CE");
  clear.click();
  assert.equal(app.expression(), "2+0");
  assert.equal(clear.textContent, "C");
  clear.click();
  assert.equal(app.expression(), "0");
  assert.equal(app.display(), "0");
  assert.equal(clear.textContent, "C");
});

test("a normal arbitrary root history entry remains an editable calculation", async () => {
  const app = workspace({
    state: {
      result: "99",
      history: [{ expression: "root(81,4)", value: 3, angle: "deg" }],
    },
  });
  const entry = app.elements.get("history").children[0].children[0];
  assert.equal(entry.tagName, "BUTTON");
  entry.click();
  assert.equal(app.expression(), "root(81,4)");
  assert.equal(app.display(), "3");
  app.responses.push({ state: { result: "3", ans: 3 } });
  app.press("equals");
  await settle();
  assert.equal(
    app.calls.find((call) => call.method === "run").fields.expression,
    "root(81,4)",
  );
  app.press("add", "1");
  assert.equal(app.expression(), "3+1");
});

test("root and integral analysis history reuse their numeric answers", () => {
  for (const expression of [
    "root(x^2-2,0,2)",
    "root(root(x,2),0,4)",
    "integral(sin(x),0,pi)",
  ]) {
    const app = workspace({
      state: {
        result: "99",
        history: [{ expression, value: 2, angle: "rad" }],
      },
    });
    const entry = app.elements.get("history").children[0].children[0];
    assert.equal(entry.tagName, "BUTTON");
    entry.click();
    assert.equal(app.expression(), "2");
    assert.equal(app.display(), "2");
    assert.equal(app.elements.get("angle").value, "rad");
    app.press("add", "1");
    assert.equal(app.expression(), "2+1");
  }
});

test("unary operations keep whitespace-separated function names with their arguments", async () => {
  const app = workspace();
  app.input("2 + sin (30)");
  app.press("sqrt");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.fields.operand, "(sqrt(sin (30)))");
  assert.equal(request.fields.expression, "2 + (sqrt(sin (30)))");
});

test("unary operations include signs separated from binary operators by whitespace", async () => {
  for (const [expression, operand] of [
    ["2 - -3", "-3"],
    ["2 + +3", "+3"],
  ]) {
    const app = workspace();
    app.input(expression);
    app.press("abs");
    await settle();
    const request = app.calls.find((call) => call.method === "run");
    assert.equal(request.fields.operand, `(abs(${operand}))`);
    assert.equal(
      request.fields.expression,
      expression.slice(0, -operand.length) + `(abs(${operand}))`,
    );
  }
});

test("draft restoration shows the current operand instead of the stale Wasm result", () => {
  for (const calculator of [
    undefined,
    {
      mode: "scientific",
      displayValue: "34",
      justEvaluated: false,
      replaceEntry: false,
    },
  ]) {
    const app = workspace({
      state: { result: "99" },
      checkpoint: {
        schema: 1,
        fields: { expression: "2+34" },
        ...(calculator ? { calculator } : {}),
      },
    });
    assert.equal(app.expression(), "2+34");
    assert.equal(app.display(), "34");
    app.press("5");
    assert.equal(app.expression(), "2+345");
    assert.equal(app.display(), "345");
  }
});

test("restoring a cleared entry preserves the C action and next-digit replacement", () => {
  for (const next of ["clear", "4"]) {
    const app = workspace({
      checkpoint: {
        schema: 1,
        fields: { expression: "2+0" },
        calculator: {
          mode: "scientific",
          displayValue: "0",
          entryCleared: true,
          replaceEntry: true,
        },
      },
    });
    const clear = app.elements
      .get("keypad")
      .children.find((button) => button.dataset.key === "clear");
    assert.equal(clear.textContent, "C");
    app.press(next);
    assert.equal(app.expression(), next === "clear" ? "0" : "2+4");
  }
});

test("standard mode has a four-column keypad with separate percent, CE and C keys", () => {
  const app = workspace({ mode: "standard" });
  assert.deepEqual(
    app.elements.get("keypad").children.map((button) => button.dataset.key),
    [
      "percent",
      "clear-entry",
      "clear-all",
      "backspace",
      "reciprocal",
      "square",
      "sqrt",
      "divide",
      "7",
      "8",
      "9",
      "multiply",
      "4",
      "5",
      "6",
      "subtract",
      "1",
      "2",
      "3",
      "add",
      "sign",
      "0",
      "decimal",
      "equals",
    ],
  );
  assert.equal(app.document.querySelector("main").dataset.mode, "standard");
  assert.equal(app.display(), "0");
});

test("standard mode completes the pending operation when the next operator is pressed", async () => {
  const app = workspace({ mode: "standard" });
  app.responses.push({ state: { result: "5" } });
  app.press("2", "add", "3", "multiply");
  await settle();
  const intermediate = app.calls.find((call) => call.method === "run");
  assert.equal(intermediate.action, "standard-evaluate");
  assert.equal(intermediate.fields.left, "2");
  assert.equal(intermediate.fields.operator, "+");
  assert.equal(intermediate.fields.right, "3");
  assert.equal(app.display(), "5");
  app.responses.push({ state: { result: "20", ans: 20 } });
  app.press("4", "equals");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "standard-calculate",
  );
  assert.equal(app.display(), "20");
});

test("standard percent uses the pending addition's left operand as its base", async () => {
  const app = workspace({ mode: "standard" });
  app.responses.push({ state: { result: "20" } });
  app.press("2", "0", "0", "add", "1", "0", "percent");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "standard-evaluate");
  assert.equal(request.fields.left, "200");
  assert.equal(request.fields.operator, "percent");
  assert.equal(request.fields.right, "10");
  assert.equal(app.display(), "20");
  app.responses.push({ state: { result: "220", ans: 220 } });
  app.press("equals");
  await settle();
  assert.equal(app.display(), "220");
});

test("standard CE preserves the pending operation and C clears it", () => {
  const app = workspace({ mode: "standard" });
  app.press("2", "add", "3", "clear-entry", "4");
  assert.equal(app.expression(), "2+4");
  app.press("clear-all", "5");
  assert.equal(app.expression(), "5");
  assert.equal(app.display(), "5");
});

test("standard mode retains the exact pasted decimal through the SDK boundary", async () => {
  const app = workspace({ mode: "standard" });
  app.event(app.document.querySelector("main"), "paste", {
    clipboardData: { getData: () => "9007199254740993" },
  });
  assert.equal(app.display(), "9,007,199,254,740,993");
  app.responses.push({
    state: { result: "9007199254740993", ans_exact: "9007199254740993" },
  });
  app.press("equals");
  await settle();
  assert.equal(
    app.calls.find((call) => call.method === "run").fields.expression,
    "9007199254740993",
  );
  assert.equal(app.display(), "9,007,199,254,740,993");
});

test("standard errors remain visible and a new digit starts a recoverable calculation", async () => {
  const app = workspace({ mode: "standard" });
  app.responses.push({ errors: { expression: "Division by zero" } });
  app.press("1", "divide", "0", "equals");
  await settle();
  assert.match(app.display(), /Division by zero/);
  app.press("2");
  assert.equal(app.display(), "2");
  assert.equal(app.expression(), "2");
});

test("standard and scientific sessions keep independent unfinished calculations", () => {
  const app = workspace();
  app.press("2", "add", "3");
  app.elements.get("standard-mode").click();
  app.press("9");
  app.elements.get("scientific-mode").click();
  assert.equal(app.expression(), "2+3");
  app.elements.get("standard-mode").click();
  assert.equal(app.display(), "9");
  assert.equal(app.expression(), "9");
});

test("the navigation drawer blocks calculation keys and Escape restores its trigger", () => {
  const app = workspace({ mode: "standard" });
  app.press("4");
  app.elements.get("navigation").click();
  app.key("7");
  assert.equal(app.display(), "4");
  assert.equal(app.elements.get("navigation-backdrop").hidden, false);
  app.key("Escape");
  assert.equal(app.elements.get("navigation-menu").hidden, true);
  assert.equal(app.document.activeElement, app.elements.get("navigation"));
});

test("Alt number shortcuts select the five calculator modes", () => {
  const app = workspace();
  for (const [key, mode] of [
    ["1", "standard"],
    ["2", "scientific"],
    ["3", "programmer"],
    ["4", "date"],
    ["5", "graph"],
  ]) {
    app.key(key, undefined, { altKey: true });
    assert.equal(app.document.querySelector("main").dataset.mode, mode);
  }
  app.key("e", undefined, { ctrlKey: true });
  assert.equal(app.document.querySelector("main").dataset.mode, "date");
});

test("memory rows retain exact values and send the selected index for row actions", async () => {
  const app = workspace({
    state: {
      memories: ["9007199254740993", "12"],
      memory: 9007199254740992,
      memory_set: true,
    },
  });
  assert.equal(app.elements.get("memory-list").children.length, 2);
  app.document.querySelector('[data-memory-recall="0"]').click();
  assert.equal(app.expression(), "9007199254740993");
  app.responses.push({
    state: { memories: ["9007199254740993"], memory_set: true },
  });
  const remove = app.document.querySelector(
    '[data-memory-action="memory-remove"][data-memory-index="1"]',
  );
  remove.click();
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "memory-remove");
  assert.equal(request.fields.memoryIndex, "1");
  assert.equal(app.elements.get("memory-list").children.length, 1);
});

test("programmer mode retains its five-column keypad and disables decimal digits outside the radix", () => {
  const app = workspace({ mode: "programmer", fields: { radix: "2" } });
  assert.equal(app.elements.get("prog-keypad").children.length, 30);
  for (const key of ["2", "7", "A", "F", "decimal"])
    assert.equal(
      app.elements
        .get("prog-keypad")
        .children.find((button) => button.dataset.key === key).disabled,
      true,
    );
  app.pressProgrammer("1", "A", "0");
  assert.equal(app.elements.get("prog-result").textContent, "10");
  assert.equal(app.elements.get("history-toggle").hidden, true);
});

test("programmer shifts use the selected logical or rotation command", async () => {
  for (const [kind, expression, keys] of [
    ["logical", "8 >>> 1", ["8", "rsh", "1", "equals"]],
    ["arithmetic", "8 >> 1", ["8", "rsh", "1", "equals"]],
    ["rotate", "ror(8)", ["8", "rsh"]],
    ["carry", "rcr(8)", ["8", "rsh"]],
  ]) {
    const app = workspace({ mode: "programmer" });
    app.document.querySelector(`[data-shift="${kind}"]`).click();
    app.pressProgrammer(...keys);
    await settle();
    const request = app.calls.find((call) => call.method === "run");
    assert.equal(request.action, "programmer-evaluate");
    assert.equal(request.fields.expression, expression);
  }
});

test("programmer word sizes cycle through all four widths", async () => {
  const app = workspace({ mode: "programmer" });
  for (const [bits, name] of [
    [32, "DWORD"],
    [16, "WORD"],
    [8, "BYTE"],
    [64, "QWORD"],
  ]) {
    app.responses.push({
      state: {
        programmer: programmerOutput({
          bits: Number(app.elements.get("bits").value),
        }),
      },
    });
    app.responses.push({
      state: {
        programmer: {
          value: "0",
          dec: "0",
          hex: "0",
          oct: "0",
          bin: "0",
          unsigned: "0",
          radix: 10,
          bits,
          carry: false,
        },
      },
    });
    app.elements.get("word-size").click();
    await settle();
    assert.equal(app.elements.get("word-size").textContent, name);
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields.bits,
      String(bits),
    );
  }
});

test("programmer bit flipping preserves all 64 positions and rejects bits above BYTE", async () => {
  const app = workspace({
    mode: "programmer",
    fields: { bits: "8" },
    state: {
      programmer: {
        value: "0",
        dec: "0",
        hex: "0",
        oct: "0",
        bin: "0",
        unsigned: "0",
        radix: 10,
        bits: 8,
        carry: false,
      },
    },
  });
  app.elements.get("bit-toggle").click();
  await settle();
  const buttons = app.elements.get("bit-grid").children;
  assert.equal(buttons.length, 64);
  assert.equal(buttons[0].dataset.bit, "63");
  assert.equal(buttons[0].disabled, true);
  const bit = buttons.find((button) => button.dataset.bit === "7");
  assert.equal(bit.disabled, false);
  app.responses.push({
    state: {
      programmer: {
        value: "-128",
        dec: "-128",
        hex: "80",
        oct: "200",
        bin: "10000000",
        unsigned: "128",
        radix: 10,
        bits: 8,
        carry: false,
      },
    },
  });
  bit.click();
  await settle();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.action, "programmer-toggle-bit");
  assert.equal(request.fields.operand, "7");
  assert.equal(app.elements.get("base-2").textContent, "10000000");
  assert.equal(
    app.document.querySelector('[data-bit="7"]').getAttribute("aria-pressed"),
    "true",
  );
});

test("date difference updates automatically and presents its calendar and total units", async () => {
  const app = workspace({ mode: "date" });
  app.responses.push({
    state: {
      date: { totalDays: 31, years: 0, months: 1, weeks: 0, days: 0, date: "" },
    },
  });
  app.elements.get("date-to").value = "2024-02-01";
  app.event(app.elements.get("date-to"), "input");
  await app.flushTimers();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "date-difference");
  assert.equal(request.fields.dateTo, "2024-02-01");
  assert.match(app.elements.get("date-result").textContent, /^1 month$/i);
  assert.equal(app.elements.get("date-total").textContent, "31 days");
});

test("date offset selects subtraction and submits independent year, month and day fields", async () => {
  const app = workspace({ mode: "date" });
  app.elements.get("date-operation").value = "add";
  app.event(app.elements.get("date-operation"), "change");
  app.elements.get("date-subtract").checked = true;
  app.event(app.elements.get("date-subtract"), "change");
  for (const [id, value] of [
    ["date-years", "1"],
    ["date-months", "2"],
    ["date-days", "3"],
  ]) {
    app.elements.get(id).value = value;
    app.event(app.elements.get(id), "input");
  }
  app.responses.push({
    state: {
      date: {
        totalDays: 0,
        years: 0,
        months: 0,
        weeks: 0,
        days: 0,
        date: "2022-10-29",
      },
    },
  });
  await app.flushTimers();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "date-offset");
  assert.equal(request.fields.dateOperation, "subtract");
  assert.equal(request.fields.dateYears, "1");
  assert.equal(request.fields.dateMonths, "2");
  assert.equal(request.fields.dateDays, "3");
  assert.equal(app.elements.get("date-to-field").hidden, true);
  assert.equal(app.elements.get("date-offset-controls").hidden, false);
  assert.equal(app.elements.get("date-result").textContent, "October 29, 2022");
});

test("date calendar selection closes the popup and restores the picker focus", () => {
  const app = workspace({ mode: "date" });
  app.elements.get("date-from-button").click();
  assert.equal(app.elements.get("calendar-popup").hidden, false);
  const selected = app.elements
    .get("calendar-grid")
    .querySelector('[data-date="2024-01-15"]');
  assert.ok(selected);
  selected.click();
  assert.equal(app.elements.get("date-from").value, "2024-01-15");
  assert.equal(app.elements.get("calendar-popup").hidden, true);
  assert.equal(
    app.document.activeElement,
    app.elements.get("date-from-button"),
  );
});

test("graph equations submit through Wasm and support independent visibility and styles", async () => {
  const app = workspace({ mode: "graph", fields: { functions: "x\nx^2" } });
  const input = app.elements
    .get("equations")
    .querySelectorAll("input[data-equation]")[1];
  input.value = "sin(x)";
  app.event(input, "input");
  app.key("Enter", input);
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "plot");
  assert.equal(request.fields.functions, "x\nsin(x)");
  const toggle = app.document.querySelector('[data-curve-toggle="0"]');
  toggle.click();
  assert.equal(toggle.getAttribute("aria-pressed"), "false");
  const style = app.document.querySelector('[data-curve-style="0"]');
  style.click();
  assert.equal(style.textContent, "┄");
});

test("a fresh graph opens with one blank equation and stays empty across mode changes", async () => {
  const app = workspace({
    mode: "standard",
    fields: { functions: "" },
    state: { plot: { series: [], xmin: -10, xmax: 10, ymin: -3, ymax: 3 } },
  });
  const assertBlankGraph = () => {
    const equations = app.elements
      .get("equations")
      .querySelectorAll("input[data-equation]");
    assert.equal(equations.length, 1);
    assert.equal(equations[0].value, "");
    assert.equal(app.elements.get("functions").value, "");
    assert.equal(app.elements.get("legend").children.length, 0);
    assert.equal(app.elements.get("error").textContent, "");
    assert.equal(
      app.calls.some((call) => call.method === "run" && call.action === "plot"),
      false,
    );
  };
  app.elements.get("graph-mode").click();
  await app.flushTimers();
  assertBlankGraph();
  app.elements.get("scientific-mode").click();
  app.elements.get("graph-mode").click();
  await app.flushTimers();
  assertBlankGraph();
});

test("narrow graph view switches between the plot and the equation keyboard", () => {
  const app = workspace({ mode: "graph" });
  app.elements.get("graph-input").click();
  assert.equal(app.document.querySelector("main").dataset.graphTab, "input");
  app.elements.get("graph-view").click();
  assert.equal(app.document.querySelector("main").dataset.graphTab, "plot");
  assert.equal(
    app.elements
      .get("keypad")
      .children.some((button) => button.dataset.key === "variable-x"),
    true,
  );
  assert.equal(
    app.elements
      .get("keypad")
      .children.some((button) => button.dataset.key === "variable-y"),
    true,
  );
});

test("standard repeat equals reuses the last operand and a fresh digit starts a new result", async () => {
  const app = workspace({ mode: "standard" });
  app.responses.push({ state: { result: "5", ans: 5 } });
  app.press("2", "add", "3", "equals");
  await settle();
  app.responses.push({ state: { result: "8", ans: 8 } });
  app.press("equals");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields.expression,
    "(2+3)+(3)",
  );
  assert.equal(app.display(), "8");
  app.press("7");
  assert.equal(app.expression(), "7");
  assert.equal(app.display(), "7");
});

test("standard operand expressions retain an exact reciprocal across later operations", async () => {
  const app = workspace({ mode: "standard" });
  app.responses.push({
    state: { result: "0.33333333333333333333333333333333" },
  });
  app.press("3", "reciprocal");
  await settle();
  app.press("multiply", "3");
  app.responses.push({ state: { result: "1" } });
  app.press("add");
  await settle();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.action, "standard-evaluate");
  assert.match(request.fields.left, /^\(?1\/\(3\)\)?$/);
  assert.equal(request.fields.right, "3");
  assert.equal(request.fields.operator, "*");
  assert.equal(app.display(), "1");
});

test("the date calendar arrows navigate across months without committing a date", () => {
  const app = workspace({ mode: "date" });
  app.elements.get("date-from-button").click();
  const first = app.elements
    .get("calendar-grid")
    .querySelector('[data-date="2024-01-01"]');
  app.key("ArrowLeft", first);
  assert.equal(app.document.activeElement.dataset.date, "2023-12-31");
  assert.equal(app.elements.get("calendar-month").value, "12");
  assert.equal(app.elements.get("calendar-year").value, "2023");
  assert.equal(app.elements.get("date-from").value, "2024-01-01");
});

test("the graph keyboard submit key runs the plot before switching to the canvas", async () => {
  const app = workspace({ mode: "graph", fields: { functions: "x^2" } });
  app.elements.get("graph-input").click();
  app.press("equals");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.ok(request, "The submit key must reach Wasm on its first activation");
  assert.equal(request.action, "plot");
  assert.equal(request.fields.functions, "x^2");
  assert.equal(app.document.querySelector("main").dataset.graphTab, "plot");
});

test("memory item additions and subtractions submit the selected slot and current operand", async () => {
  for (const action of ["memory-add", "memory-subtract"]) {
    const app = workspace({
      state: { memory: 10, memory_set: true, memories: ["10", "20"] },
    });
    app.press("5");
    app.document
      .querySelector(`[data-memory-action="${action}"][data-memory-index="1"]`)
      .click();
    await settle();
    const request = app.calls.find((call) => call.method === "run");
    assert.equal(request.action, action);
    assert.equal(request.fields.memoryIndex, "1");
    assert.equal(request.fields.operand, "5");
  }
});

test("history recalls use the exact decimal text instead of its approximate numeric companion", () => {
  const app = workspace({
    state: {
      history: [
        {
          expression: "9007199254740992+1",
          value: 9007199254740992,
          exact: "9007199254740993",
          angle: "deg",
          mode: "scientific",
        },
      ],
    },
  });
  app.elements.get("history").children[0].children[0].click();
  assert.equal(app.display(), "9,007,199,254,740,993");
  app.press("add", "1");
  assert.equal(app.expression(), "9007199254740993+1");
});

test("switching away from date calculation cancels a pending automatic recalculation", async () => {
  const app = workspace({ mode: "date" });
  app.elements.get("date-to").value = "2024-02-01";
  app.event(app.elements.get("date-to"), "input");
  app.elements.get("scientific-mode").click();
  app.press("5");
  await app.flushTimers();
  assert.equal(
    app.calls.some(
      (call) => call.method === "run" && call.action.startsWith("date-"),
    ),
    false,
  );
  assert.equal(app.expression(), "5");
});

test("switching away from graph cancels a pending equation change submission", async () => {
  const app = workspace({ mode: "graph", fields: { functions: "x" } });
  const input = app.elements
    .get("equations")
    .querySelector("input[data-equation]");
  input.value = "x^2";
  app.event(input, "input");
  app.event(input, "change");
  app.elements.get("scientific-mode").click();
  app.press("5");
  await app.flushTimers();
  assert.equal(
    app.calls.some((call) => call.method === "run" && call.action === "plot"),
    false,
  );
  assert.equal(app.expression(), "5");
});

test("programmer base conversion preserves a negative decimal prefix", async () => {
  const negative = programmerOutput({
    value: "-1",
    dec: "-1",
    hex: "FFFFFFFFFFFFFFFF",
    oct: "1777777777777777777777",
    bin: "1".repeat(64),
    unsigned: "18446744073709551615",
  });
  const app = workspace({
    mode: "programmer",
    state: { programmer: negative },
  });
  app.responses.push({ state: { programmer: negative } });
  app.responses.push({
    state: {
      programmer: { ...negative, value: "FFFFFFFFFFFFFFFF", radix: 16 },
    },
  });
  app.document.querySelector('[data-base="16"]').click();
  await settle();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.fields.expression, "-0d1");
  assert.equal(request.fields.radix, "16");
  assert.equal(app.elements.get("prog-result").textContent, "FFFFFFFFFFFFFFFF");
});

test("programmer memory recall uses a signed decimal literal in non-decimal bases", async () => {
  const app = workspace({
    mode: "programmer",
    fields: { radix: "16" },
    state: { memory: -1, memory_set: true, memories: ["-1"] },
  });
  app.document.querySelector('[data-memory-recall="0"]').click();
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "programmer-evaluate");
  assert.equal(request.fields.expression, "-0d1");
  assert.equal(request.fields.radix, "16");
});

test("programmer unary operations preserve a pending binary expression", async () => {
  const cases = [
    {
      key: "sign",
      command: "neg(3)",
      value: "-3",
      hex: "FFFFFFFFFFFFFFFD",
      oct: "1777777777777777777775",
      bin: "1".repeat(62) + "01",
      unsigned: "18446744073709551613",
    },
    {
      key: "not",
      command: "not(3)",
      value: "-4",
      hex: "FFFFFFFFFFFFFFFC",
      oct: "1777777777777777777774",
      bin: "1".repeat(62) + "00",
      unsigned: "18446744073709551612",
    },
    {
      key: "rotate",
      command: "ror(3)",
      value: "-9223372036854775807",
      hex: "8000000000000001",
      oct: "1000000000000000000001",
      bin: "1" + "0".repeat(62) + "1",
      unsigned: "9223372036854775809",
    },
  ];
  for (const entry of cases) {
    const { key, command, ...output } = entry;
    const app = workspace({ mode: "programmer" });
    app.pressProgrammer("5", "add", "3");
    app.responses.push({
      state: { programmer: programmerOutput({ ...output, dec: entry.value }) },
    });
    if (key === "not")
      app.document.querySelector('[data-prog-op="not"]').click();
    else if (key === "rotate") {
      app.document.querySelector('[data-shift="rotate"]').click();
      app.pressProgrammer("rsh");
    } else app.pressProgrammer(key);
    await settle();
    assert.equal(
      app.calls.find((call) => call.method === "run").fields.expression,
      command,
    );
    assert.equal(app.expression(), "5+" + entry.value);
    app.pressProgrammer("equals");
    await settle();
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields
        .expression,
      "5+" + entry.value,
    );
  }
});

test("programmer consecutive operators replace the unfinished operator", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add", "multiply", "3", "equals");
  await settle();
  const requests = app.calls.filter((call) => call.method === "run");
  assert.equal(requests.length, 1);
  assert.equal(requests[0].fields.expression, "5*3");
});

test("programmer waiting operators survive session switching and checkpoint restoration", async () => {
  for (const restore of [false, true]) {
    let app = workspace({ mode: "programmer" });
    app.pressProgrammer("5", "add");
    await app.flushTimers();
    const checkpoint = app.calls
      .filter((call) => call.method === "checkpoint")
      .at(-1).value;
    assert.equal(checkpoint.calculator.programmerWaiting, true);
    assert.equal(
      checkpoint.calculator.modeSessions.programmer.programmerWaiting,
      true,
    );
    if (restore) app = workspace({ mode: "programmer", checkpoint });
    else {
      app.elements.get("scientific-mode").click();
      app.elements.get("programmer-mode").click();
    }
    app.pressProgrammer("multiply", "3", "equals");
    await settle();
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields
        .expression,
      "5*3",
    );
  }
});

test("programmer C calls the carry-clearing action", async () => {
  const app = workspace({
    mode: "programmer",
    state: { carry: true, programmer: programmerOutput({ carry: true }) },
  });
  app.responses.push({
    state: { carry: false, programmer: programmerOutput() },
  });
  app.pressProgrammer("clear-all");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "programmer-clear");
  assert.equal(request.fields.expression, "0");
  assert.equal(app.elements.get("prog-result").textContent, "0");
});

test("programmer memory controls preview the current operand before using its exact decimal value", async () => {
  for (const control of [
    "toolbar",
    "memory-store",
    "memory-add",
    "memory-subtract",
  ]) {
    const app = workspace({
      mode: "programmer",
      fields: { radix: "16" },
      state: {
        memory: 10,
        memory_set: true,
        memories: ["10"],
        programmer: programmerOutput({ radix: 16 }),
      },
    });
    app.pressProgrammer("5", "add", "F", "F");
    app.responses.push({
      state: {
        programmer: programmerOutput({
          value: "FF",
          dec: "255",
          unsigned: "255",
          hex: "FF",
          oct: "377",
          bin: "11111111",
          radix: 16,
        }),
      },
    });
    if (control === "toolbar") app.elements.get("prog-memory-store").click();
    else if (control === "memory-store") app.action(control);
    else
      app.document
        .querySelector(
          `[data-memory-action="${control}"][data-memory-index="0"]`,
        )
        .click();
    await settle();
    const requests = app.calls.filter((call) => call.method === "run");
    assert.equal(requests.length, 2);
    assert.equal(requests[0].action, "programmer-preview");
    assert.equal(requests[0].fields.operand, "FF");
    assert.equal(requests[0].fields.expression, "5+FF");
    assert.equal(
      requests[1].action,
      control === "toolbar" ? "memory-store" : control,
    );
    assert.equal(requests[1].fields.operand, "255");
    if (["memory-add", "memory-subtract"].includes(control))
      assert.equal(requests[1].fields.memoryIndex, "0");
    assert.equal(app.expression(), "5+FF");
  }
});

test("entering the programmer bit view previews only the operand and keeps its pending formula", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add", "3");
  app.responses.push({
    state: {
      programmer: programmerOutput({
        value: "3",
        dec: "3",
        hex: "3",
        oct: "3",
        bin: "11",
        unsigned: "3",
      }),
    },
  });
  app.elements.get("bit-toggle").click();
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "programmer-preview");
  assert.equal(request.fields.operand, "3");
  assert.equal(request.fields.expression, "5+3");
  assert.equal(app.expression(), "5+3");
  assert.equal(app.elements.get("prog-keypad").hidden, true);
  assert.equal(
    app.document.querySelector('[data-bit="0"]').getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(
    app.document.querySelector('[data-bit="1"]').getAttribute("aria-pressed"),
    "true",
  );
});

test("mode fields remain consistent when leaving date after an acknowledged action", async () => {
  const app = workspace({ mode: "date" });
  app.responses.push({
    state: {
      date: { totalDays: 31, years: 0, months: 1, weeks: 0, days: 0, date: "" },
    },
  });
  app.elements.get("date-to").value = "2024-02-01";
  app.event(app.elements.get("date-to"), "input");
  await app.flushTimers();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "date-difference",
  );
  app.elements.get("scientific-mode").click();
  assert.equal(app.document.querySelector("main").dataset.mode, "scientific");
  assert.equal(app.elements.get("mode").value, "scientific");
  app.responses.push({ state: { result: "5", ans: 5 } });
  app.press("2", "add", "3", "equals");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields.mode,
    "scientific",
  );
});

test("loading a workspace restores its own mode and recovery checkpoint", async () => {
  const app = workspace({ permissions: ["storage"] });
  app.press("7", "add", "1");
  const loadedFields = {
    ...app.snapshot.fields,
    mode: "date",
    expression: "0",
    dateFrom: "2024-01-01",
    dateTo: "2024-02-01",
  };
  app.responses.push({
    state: {
      fields: loadedFields,
      result: "0",
      date: { totalDays: 31, years: 0, months: 1, weeks: 0, days: 0, date: "" },
    },
  });
  app.elements.get("load").click();
  app.elements.get("load-confirm-button").click();
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "load",
  );
  assert.equal(app.document.querySelector("main").dataset.mode, "date");
  assert.equal(app.elements.get("mode").value, "date");
  assert.equal(app.elements.get("date-panel").hidden, false);
  assert.equal(app.elements.get("date-to").value, "2024-02-01");
  assert.equal(
    app.calls.filter((call) => call.method === "checkpoint").at(-1).value
      .calculator.mode,
    "date",
  );
});

test("programmer base conversion previews only the current operand and retains pending addition", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add", "3");
  const three = programmerOutput({
    value: "3",
    dec: "3",
    hex: "3",
    oct: "3",
    bin: "11",
    unsigned: "3",
  });
  app.responses.push({ state: { programmer: three } });
  app.responses.push({ state: { programmer: { ...three, radix: 16 } } });
  app.document.querySelector('[data-base="16"]').click();
  await settle();
  const previews = app.calls.filter((call) => call.method === "run");
  assert.equal(previews.length, 2);
  assert.deepEqual(
    previews.map((call) => call.action),
    ["programmer-preview", "programmer-preview"],
  );
  assert.equal(previews[0].fields.operand, "3");
  assert.equal(previews[0].fields.radix, "10");
  assert.equal(previews[1].fields.operand, "0d3");
  assert.equal(previews[1].fields.radix, "16");
  assert.equal(app.elements.get("prog-result").textContent, "3");
  app.responses.push({
    state: {
      programmer: programmerOutput({
        value: "8",
        dec: "8",
        hex: "8",
        oct: "10",
        bin: "1000",
        unsigned: "8",
        radix: 16,
      }),
    },
  });
  app.pressProgrammer("equals");
  await settle();
  const expression = app.calls
    .filter((call) => call.method === "run")
    .at(-1)
    .fields.expression.replace(/[()\s]/g, "");
  assert.match(expression, /^(?:0d)?5\+(?:0d)?3$/);
  assert.equal(app.elements.get("prog-result").textContent, "8");
});

test("programmer word conversion previews the old and new width without consuming the formula", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add", "3");
  const three = programmerOutput({
    value: "3",
    dec: "3",
    hex: "3",
    oct: "3",
    bin: "11",
    unsigned: "3",
  });
  app.responses.push({ state: { programmer: three } });
  app.responses.push({ state: { programmer: { ...three, bits: 32 } } });
  app.elements.get("word-size").click();
  await settle();
  const previews = app.calls.filter((call) => call.method === "run");
  assert.deepEqual(
    previews.map((call) => call.action),
    ["programmer-preview", "programmer-preview"],
  );
  assert.equal(previews[0].fields.bits, "64");
  assert.equal(previews[0].fields.operand, "3");
  assert.equal(previews[1].fields.bits, "32");
  assert.equal(previews[1].fields.operand, "0d3");
  app.responses.push({
    state: {
      programmer: programmerOutput({
        value: "8",
        dec: "8",
        hex: "8",
        oct: "10",
        bin: "1000",
        unsigned: "8",
        bits: 32,
      }),
    },
  });
  app.pressProgrammer("equals");
  await settle();
  const expression = app.calls
    .filter((call) => call.method === "run")
    .at(-1)
    .fields.expression.replace(/[()\s]/g, "");
  assert.match(expression, /^(?:0d)?5\+(?:0d)?3$/);
  assert.equal(app.elements.get("prog-result").textContent, "8");
});

test("widening HEX BYTE FF sign extends negative one to QWORD", async () => {
  const byte = programmerOutput({
    value: "FF",
    dec: "-1",
    hex: "FF",
    oct: "377",
    bin: "11111111",
    unsigned: "255",
    radix: 16,
    bits: 8,
  });
  const qword = programmerOutput({
    value: "FFFFFFFFFFFFFFFF",
    dec: "-1",
    hex: "FFFFFFFFFFFFFFFF",
    oct: "1777777777777777777777",
    bin: "1".repeat(64),
    unsigned: "18446744073709551615",
    radix: 16,
  });
  const app = workspace({
    mode: "programmer",
    fields: { radix: "16", bits: "8" },
    state: { programmer: byte },
  });
  app.responses.push({ state: { programmer: byte } });
  app.responses.push({ state: { programmer: qword } });
  app.elements.get("word-size").click();
  await settle();
  const requests = app.calls.filter((call) => call.method === "run");
  assert.equal(requests[0].action, "programmer-preview");
  assert.equal(requests[0].fields.bits, "8");
  assert.equal(requests[0].fields.operand, "FF");
  assert.equal(requests[1].action, "programmer-preview");
  assert.equal(requests[1].fields.bits, "64");
  assert.equal(requests[1].fields.operand, "-0d1");
  assert.equal(app.elements.get("prog-result").textContent, "FFFFFFFFFFFFFFFF");
  assert.equal(app.elements.get("base-10").textContent, "-1");
});

test("programmer operator waiting survives a base change and still allows operator replacement", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add");
  const five = programmerOutput({
    value: "5",
    dec: "5",
    hex: "5",
    oct: "5",
    bin: "101",
    unsigned: "5",
  });
  app.responses.push({ state: { programmer: five } });
  app.responses.push({ state: { programmer: { ...five, radix: 16 } } });
  app.document.querySelector('[data-base="16"]').click();
  await settle();
  await app.flushTimers();
  const saved = app.calls.filter((call) => call.method === "checkpoint").at(-1)
    .value.calculator;
  assert.equal(saved.programmerWaiting, true);
  assert.equal(saved.modeSessions.programmer.programmerWaiting, true);
  app.responses.push({
    state: {
      programmer: programmerOutput({
        value: "F",
        dec: "15",
        hex: "F",
        oct: "17",
        bin: "1111",
        unsigned: "15",
        radix: 16,
      }),
    },
  });
  app.pressProgrammer("multiply", "3", "equals");
  await settle();
  const expression = app.calls
    .filter((call) => call.method === "run")
    .at(-1)
    .fields.expression.replace(/[()\s]/g, "");
  assert.match(expression, /^(?:0d)?5\*(?:0d)?3$/);
  assert.equal(app.elements.get("prog-result").textContent, "F");
});

function converterOutput(overrides = {}) {
  return {
    category: "length",
    from: "34",
    to: "30",
    input: "2",
    result: "200",
    supportsNegative: false,
    units: [
      { id: "34", label: "Meters", abbreviation: "m", whimsical: false },
      { id: "30", label: "Centimeters", abbreviation: "cm", whimsical: false },
      { id: "32", label: "Inches", abbreviation: "in", whimsical: false },
    ],
    equivalents: [],
    updatedAt: null,
    nextUpdateAt: null,
    rate: null,
    source: null,
    ...overrides,
  };
}

function converterWorkspace(data = converterOutput(), options = {}) {
  return workspace({
    ...options,
    mode: "converter",
    fields: {
      converterCategory: data.category,
      converterFrom: data.from,
      converterTo: data.to,
      converterInput: data.input,
      ...options.fields,
    },
    state: { converter: data, ...options.state },
  });
}

// Explicit native receipts keep these tests focused on UI routing, selection,
// and draft restoration. Rust tests verify the conversion arithmetic.
function converterReceipt(app, data) {
  return {
    state: {
      converter: data,
      fields: {
        ...app.snapshot.fields,
        mode: "converter",
        converterCategory: data.category,
        converterFrom: data.from,
        converterTo: data.to,
        converterInput: data.input,
      },
    },
  };
}

test("loading a converter workspace checkpoints the loaded values before creating mode sessions", async () => {
  const app = converterWorkspace(
    converterOutput({ input: "123", result: "12300" }),
    { permissions: ["storage"], fields: { expression: "123" } },
  );
  const temperature = converterOutput({
    category: "temperature",
    from: "46",
    to: "47",
    input: "100",
    result: "212",
    supportsNegative: true,
    units: [
      { id: "46", label: "Celsius", abbreviation: "°C", whimsical: false },
      { id: "47", label: "Fahrenheit", abbreviation: "°F", whimsical: false },
    ],
  });
  const receipt = converterReceipt(app, temperature);
  receipt.state.fields.expression = "100";
  receipt.state.result = "100";
  app.responses.push(receipt);
  app.elements.get("load").click();
  app.elements.get("load-confirm-button").click();
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "load",
  );
  const saved = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1).value;
  assert.equal(saved.fields.converterCategory, "temperature");
  assert.equal(saved.fields.converterInput, "100");
  assert.equal(saved.calculator.modeSessions.converter.expression, "100");
  assert.equal(saved.calculator.modeSessions.converter.displayValue, "100");
  assert.deepEqual(saved.calculator.converterSessions.temperature, {
    from: "46",
    to: "47",
    input: "100",
    side: "from",
    replace: true,
  });
  assert.equal(saved.calculator.converterSessions.length, undefined);
  app.elements.get("scientific-mode").click();
  app.document.querySelector('[data-converter-category="temperature"]').click();
  assert.equal(app.elements.get("mode").value, "converter");
  assert.equal(app.elements.get("converterCategory").value, "temperature");
  assert.equal(app.elements.get("converterFrom").value, "46");
  assert.equal(app.elements.get("converterTo").value, "47");
  assert.equal(app.elements.get("converterInput").value, "100");
  assert.equal(app.expression(), "100");
  assert.equal(app.elements.get("converter-output-from").textContent, "100");
  assert.equal(app.elements.get("converter-output-to").textContent, "212");
});

test("navigation opens all thirteen converter categories in Windows order", () => {
  const app = workspace({ mode: "standard" });
  const categories = [
    "currency",
    "volume",
    "length",
    "weight",
    "temperature",
    "energy",
    "area",
    "speed",
    "time",
    "power",
    "data",
    "pressure",
    "angle",
  ];
  const buttons = app.document.querySelectorAll("[data-converter-category]");
  assert.deepEqual(
    buttons.map((node) => node.dataset.converterCategory),
    categories,
  );
  for (const button of buttons) {
    button.click();
    assert.equal(app.document.querySelector("main").dataset.mode, "converter");
    assert.equal(app.elements.get("mode").value, "converter");
    assert.equal(
      app.elements.get("converterCategory").value,
      button.dataset.converterCategory,
    );
    assert.equal(app.elements.get("converter-panel").hidden, false);
    assert.equal(button.getAttribute("aria-current"), "page");
    assert.equal(app.elements.get("history-toggle").hidden, true);
  }
});

test("converter keypad has three columns of numeric entry controls without equals", () => {
  const app = converterWorkspace();
  const keys = app.elements
    .get("converter-keypad")
    .querySelectorAll("[data-converter-key]")
    .map((node) => node.dataset.converterKey);
  assert.deepEqual(keys, [
    "clear-entry",
    "backspace",
    "7",
    "8",
    "9",
    "4",
    "5",
    "6",
    "1",
    "2",
    "3",
    "negate",
    "0",
    ".",
  ]);
  assert.equal(keys.includes("equals"), false);
  app.key("Enter");
  assert.equal(
    app.calls.some((call) => call.method === "run"),
    false,
  );
});

test("converter entry automatically submits its four fields and renders the explicit result", async () => {
  const app = converterWorkspace();
  app.responses.push(
    converterReceipt(app, converterOutput({ input: "12.5", result: "1250" })),
  );
  app.pressConverter("1", "2", ".", "5");
  assert.equal(app.elements.get("converter-output-from").textContent, "12.5");
  assert.equal(app.elements.get("converter-output-to").textContent, "—");
  await app.flushTimers();
  const calls = app.calls.filter((call) => call.method === "run");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].action, "converter-convert");
  for (const [key, value] of Object.entries({
    converterCategory: "length",
    converterFrom: "34",
    converterTo: "30",
    converterInput: "12.5",
  }))
    assert.equal(calls[0].fields[key], value);
  assert.equal(app.elements.get("converter-output-to").textContent, "1,250");
});

test("activating the lower value reverses logical units while keeping their visual positions", async () => {
  const app = converterWorkspace();
  app.responses.push(
    converterReceipt(
      app,
      converterOutput({ from: "30", to: "34", input: "200", result: "2" }),
    ),
  );
  app.elements.get("converter-value-to").click();
  await app.flushTimers();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.fields.converterFrom, "30");
  assert.equal(request.fields.converterTo, "34");
  assert.equal(request.fields.converterInput, "200");
  assert.equal(app.elements.get("converter-from").value, "34");
  assert.equal(app.elements.get("converter-to").value, "30");
  assert.equal(
    app.elements.get("converter-value-to").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.elements.get("converter-output-from").textContent, "2");
  app.pressConverter("7");
  assert.equal(app.elements.get("converterInput").value, "7");
  assert.equal(app.elements.get("converter-output-to").textContent, "7");
});

test("changing a unit preserves the active side and maps the visual target correctly", async () => {
  const app = converterWorkspace();
  app.elements.get("converter-value-to").click();
  app.responses.push(
    converterReceipt(
      app,
      converterOutput({
        from: "30",
        to: "32",
        input: "200",
        result: "78.74015748031496",
      }),
    ),
  );
  const select = app.elements.get("converter-from");
  select.value = "32";
  app.event(select, "change");
  await app.flushTimers();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.fields.converterFrom, "30");
  assert.equal(request.fields.converterTo, "32");
  assert.equal(request.fields.converterInput, "200");
  assert.equal(
    app.elements.get("converter-value-to").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(app.elements.get("converter-from").value, "32");
  assert.equal(app.elements.get("converter-to").value, "30");
});

test("negative entry is available only for converter categories that support it", () => {
  const length = converterWorkspace();
  assert.equal(length.elements.get("converter-negate").hidden, true);
  length.key("F9");
  assert.equal(length.elements.get("converterInput").value, "2");
  const temperature = converterWorkspace(
    converterOutput({
      category: "temperature",
      supportsNegative: true,
      input: "20",
      result: "68",
    }),
  );
  assert.equal(temperature.elements.get("converter-negate").hidden, false);
  temperature.key("F9");
  assert.equal(temperature.elements.get("converterInput").value, "-20");
  temperature.key("F9");
  assert.equal(temperature.elements.get("converterInput").value, "20");
});

test("converter typing stops at sixteen digits and retains a trailing decimal point", async () => {
  const app = converterWorkspace();
  app.pressConverter(..."12345678901234567");
  assert.equal(app.elements.get("converterInput").value, "1234567890123456");
  app.pressConverter("clear-entry", "1", "2", ".", ".");
  app.responses.push(
    converterReceipt(app, converterOutput({ input: "12.", result: "1200" })),
  );
  await app.flushTimers();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields
      .converterInput,
    "12.",
  );
  assert.equal(app.elements.get("converterInput").value, "12.");
  assert.equal(app.elements.get("converter-output-from").textContent, "12.");
});

test("converter paste accepts numeric text without discarding the decimal draft", async () => {
  const app = converterWorkspace();
  const event = app.event(app.document.querySelector("main"), "paste", {
    clipboardData: { getData: () => "+1,234." },
  });
  assert.equal(event.defaultPrevented, true);
  assert.equal(app.elements.get("converterInput").value, "1234.");
  app.responses.push(
    converterReceipt(
      app,
      converterOutput({ input: "1234.", result: "123400" }),
    ),
  );
  await app.flushTimers();
  assert.equal(app.elements.get("converter-output-from").textContent, "1,234.");
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields
      .converterInput,
    "1234.",
  );
  app.event(app.document.querySelector("main"), "paste", {
    clipboardData: { getData: () => "2+3" },
  });
  assert.equal(app.elements.get("converterInput").value, "1234.");
});

test("converter categories keep separate entry, units, and active-side sessions", async () => {
  const app = converterWorkspace();
  app.elements.get("converter-value-to").click();
  app.pressConverter("7");
  app.document.querySelector('[data-converter-category="temperature"]').click();
  const temperature = converterOutput({
    category: "temperature",
    from: "46",
    to: "47",
    input: "0",
    result: "32",
    supportsNegative: true,
  });
  app.responses.push(converterReceipt(app, temperature));
  await app.flushTimers();
  app.pressConverter("9");
  app.document.querySelector('[data-converter-category="length"]').click();
  assert.equal(app.elements.get("converterInput").value, "7");
  assert.equal(app.elements.get("converterFrom").value, "30");
  assert.equal(app.elements.get("converterTo").value, "34");
  assert.equal(
    app.elements.get("converter-value-to").getAttribute("aria-pressed"),
    "true",
  );
  app.document.querySelector('[data-converter-category="temperature"]').click();
  assert.equal(app.elements.get("converterInput").value, "9");
  assert.equal(app.elements.get("converterFrom").value, "46");
  assert.equal(app.elements.get("converterTo").value, "47");
  assert.equal(
    app.elements.get("converter-value-from").getAttribute("aria-pressed"),
    "true",
  );
});

test("leaving converter mode cancels delayed conversion submission", async () => {
  const app = converterWorkspace();
  app.pressConverter("8");
  app.elements.get("scientific-mode").click();
  await app.flushTimers();
  assert.equal(app.document.querySelector("main").dataset.mode, "scientific");
  assert.equal(
    app.calls.some(
      (call) => call.method === "run" && call.action === "converter-convert",
    ),
    false,
  );
});

test("converter checkpoint restores the lower active input and its replacement state", async () => {
  const app = converterWorkspace();
  app.elements.get("converter-value-to").click();
  app.pressConverter("7");
  const data = converterOutput({
    from: "30",
    to: "34",
    input: "7",
    result: "0.07",
  });
  app.responses.push(converterReceipt(app, data));
  await app.flushTimers();
  const saved = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1).value;
  const restored = converterWorkspace(data, { checkpoint: saved });
  assert.equal(restored.elements.get("converterInput").value, "7");
  assert.equal(
    restored.elements.get("converter-value-to").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(restored.elements.get("converter-output-to").textContent, "7");
  assert.equal(restored.elements.get("converter-from").value, "34");
  restored.pressConverter("8");
  assert.equal(restored.elements.get("converterInput").value, "78");
});

test("converter scientific notation keeps its exponent intact in results and pasted input", async () => {
  const app = converterWorkspace(converterOutput({ result: "1e1000" }));
  assert.equal(app.elements.get("converter-output-to").textContent, "1e1000");
  app.event(app.document.querySelector("main"), "paste", {
    clipboardData: { getData: () => "1e1000" },
  });
  app.responses.push(
    converterReceipt(
      app,
      converterOutput({ input: "1e1000", result: "1e1000" }),
    ),
  );
  await app.flushTimers();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields
      .converterInput,
    "1e1000",
  );
  assert.equal(app.elements.get("converter-output-from").textContent, "1e1000");
  assert.equal(app.elements.get("converter-output-to").textContent, "1e1000");
});

test("whimsical units appear as supplementary comparisons rather than selectable units", () => {
  const data = converterOutput();
  data.units.push({
    id: "105",
    label: "Paperclips",
    abbreviation: "paperclips",
    whimsical: true,
  });
  data.equivalents = [
    {
      id: "105",
      label: "Paperclips",
      abbreviation: "paperclips",
      value: "62.5",
    },
  ];
  const app = converterWorkspace(data);
  for (const id of ["converter-from", "converter-to"])
    assert.equal(
      app.elements.get(id).children.some((option) => option.value === "105"),
      false,
    );
  const equivalent = app.elements.get("converter-equivalents").children[0];
  assert.equal(equivalent.children[0].textContent, "62.5");
  assert.equal(equivalent.children[1].textContent, "paperclips");
});

test("supplementary resize skips an oversized middle result and restores whole items when widened", () => {
  const app = converterWorkspace(
    converterOutput({
      equivalents: [
        { id: "40", label: "Yards", abbreviation: "yd", value: "1.09" },
        { id: "31", label: "Feet", abbreviation: "ft", value: "3.28" },
        { id: "32", label: "Inches", abbreviation: "in", value: "39.37" },
        { id: "37", label: "Millimeters", abbreviation: "mm", value: "1000" },
      ],
    }),
  );
  const container = app.elements.get("converter-equivalents"),
    items = [...container.children],
    widths = [80, 300, 60, 40];
  container.clientWidth = 240;
  items.forEach((item, index) => {
    item.getBoundingClientRect = () => ({
      width: item.hidden ? 0 : widths[index],
    });
  });
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, true, false, false],
  );
  assert.equal(items[1].children[0].textContent, "3.28");
  container.clientWidth = 600;
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, false, false, false],
  );
  assert.equal(container.children[1], items[1]);
  container.clientWidth = 240;
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, true, false, false],
  );
});

test("supplementary resize reserves space for the final whimsical result without blocking ordinary results when it cannot fit", () => {
  const data = converterOutput({
    equivalents: [
      { id: "40", label: "Yards", abbreviation: "yd", value: "1.09" },
      { id: "31", label: "Feet", abbreviation: "ft", value: "3.28" },
      { id: "32", label: "Inches", abbreviation: "in", value: "39.37" },
      { id: "131", label: "Hands", abbreviation: "hands", value: "5.36" },
    ],
  });
  data.units.push({
    id: "131",
    label: "Hands",
    abbreviation: "hands",
    whimsical: true,
  });
  const app = converterWorkspace(data),
    container = app.elements.get("converter-equivalents"),
    items = [...container.children],
    widths = [80, 60, 40, 70];
  container.clientWidth = 240;
  items.forEach((item, index) => {
    item.getBoundingClientRect = () => ({
      width: item.hidden ? 0 : widths[index],
    });
  });
  assert.equal(items[3].dataset.whimsical, "true");
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, true, false, false],
  );
  assert.equal(items[3].children[0].textContent, "5.36");
  widths[3] = 300;
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, false, false, true],
  );
  container.clientWidth = 600;
  app.resize(container);
  assert.deepEqual(
    items.map((item) => item.hidden),
    [false, false, false, false],
  );
});

test("currency without cached rates shows an unavailable result and respects refresh permissions", () => {
  const app = converterWorkspace(
    converterOutput({
      category: "currency",
      from: "USD",
      to: "EUR",
      input: "0",
      result: null,
      units: [{ id: "USD" }, { id: "EUR" }],
    }),
    { disabledActions: ["currency-refresh"] },
  );
  assert.equal(app.elements.get("converter-output-to").textContent, "—");
  assert.equal(app.elements.get("converter-value-to").disabled, true);
  assert.equal(app.elements.get("currency-info").hidden, false);
  assert.equal(app.elements.get("converter-equivalent-section").hidden, true);
  assert.equal(app.elements.get("currency-date").textContent, "");
  assert.equal(
    app.elements.get("currency-status").textContent,
    "Update rates to convert",
  );
  assert.equal(app.elements.get("currency-refresh").disabled, true);
  app.elements.get("currency-refresh").click();
  assert.equal(
    app.calls.some((call) => call.method === "run"),
    false,
  );
});

test("currency refresh uses its declared action and attribution opens through the links service", async () => {
  const app = converterWorkspace(
    converterOutput({
      category: "currency",
      from: "USD",
      to: "EUR",
      input: "2",
      result: null,
      units: [{ id: "USD" }, { id: "EUR" }],
    }),
  );
  app.responses.push(
    converterReceipt(
      app,
      converterOutput({
        category: "currency",
        from: "USD",
        to: "EUR",
        input: "2",
        result: "1.8",
        rate: "0.9",
        updatedAt: 1,
        nextUpdateAt: 2,
        units: [{ id: "USD" }, { id: "EUR" }],
      }),
    ),
  );
  app.elements.get("currency-refresh").click();
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "currency-refresh",
  );
  assert.equal(app.elements.get("converter-output-to").textContent, "1.8");
  assert.equal(
    app.elements.get("currency-rate").textContent,
    "1 USD = 0.9 EUR",
  );
  assert.equal(
    app.elements.get("currency-status").textContent,
    "Using cached rates",
  );
  app.elements.get("currency-source").click();
  await settle();
  const request = app.calls.find((call) => call.method === "service");
  assert.equal(request.service, "links");
  assert.deepEqual(request.payload, {
    url: "https://www.exchangerate-api.com",
  });
});

test("failed currency refresh preserves an explicit cached conversion and timestamp", async () => {
  const app = converterWorkspace(
    converterOutput({
      category: "currency",
      from: "USD",
      to: "EUR",
      input: "2",
      result: "1.8",
      rate: "0.9",
      updatedAt: 1,
      nextUpdateAt: 2,
      units: [{ id: "USD" }, { id: "EUR" }],
    }),
  );
  const timestamp = app.elements.get("currency-date").textContent;
  app.responses.push({
    errors: { converterInput: "Rate refresh unavailable" },
  });
  app.elements.get("currency-refresh").click();
  await settle();
  assert.equal(app.elements.get("converter-output-to").textContent, "1.8");
  assert.equal(app.elements.get("currency-date").textContent, timestamp);
  assert.equal(
    app.elements.get("error").textContent,
    "Rate refresh unavailable",
  );
});

test("formula paste preserves argument commas while whole grouped numbers are normalized", async () => {
  for (const [pasted, expected] of [
    ["root(27,3)", "root(27,3)"],
    ["log(8,2)", "log(8,2)"],
    ["max(1,234)", "max(1,234)"],
    ["1,234.50", "1234.50"],
    ["-1,234,567.5e-2", "-1234567.5e-2"],
    ["1,23", "1,23"],
  ]) {
    const app = workspace();
    app.event(app.document.querySelector("main"), "paste", {
      clipboardData: { getData: () => pasted },
    });
    app.press("equals");
    await settle();
    assert.equal(
      app.calls.find((call) => call.method === "run").fields.expression,
      expected,
    );
  }
});

test("programmer paste starts a fresh formula without retaining an unfinished operator", async () => {
  const app = workspace({ mode: "programmer" });
  app.pressProgrammer("5", "add");
  app.event(app.document.querySelector("main"), "paste", {
    clipboardData: { getData: () => "3" },
  });
  app.pressProgrammer("multiply", "2", "equals");
  await settle();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.fields.expression, "3*2");
  assert.equal(request.fields.operand, "");
});

test("copy and paste preserve native editing behavior in inputs textareas and editable descendants", () => {
  const app = workspace({ mode: "graph" });
  const editor = app.document.createElement("div");
  editor.setAttribute("contenteditable", "true");
  const descendant = app.document.createElement("span");
  editor.append(descendant);
  for (const target of [
    app.elements.get("equations").querySelector("input"),
    app.elements.get("functions"),
    descendant,
  ]) {
    const copied = [];
    const copy = app.event(target, "copy", {
      clipboardData: { setData: (...value) => copied.push(value) },
    });
    assert.equal(copy.defaultPrevented, false);
    assert.equal(copied.length, 0);
    const paste = app.event(target, "paste", {
      clipboardData: { getData: () => "sin(x)" },
    });
    assert.equal(paste.defaultPrevented, false);
    assert.equal(app.expression(), "0");
  }
});

test("copy outside an editor still copies the displayed calculator operand", () => {
  const app = workspace();
  app.press("8");
  const copied = [];
  const event = app.event(app.document.querySelector("main"), "copy", {
    clipboardData: { setData: (...value) => copied.push(value) },
  });
  assert.equal(event.defaultPrevented, true);
  assert.deepEqual(copied, [["text/plain", "8"]]);
});

test("pasting while dialogs or navigation overlays are open leaves the background calculation intact", () => {
  for (const id of [
    "settings-dialog",
    "load-confirm",
    "calendar-popup",
    "graph-settings",
    "navigation-menu",
    "trig-popup",
    "functions-popup",
  ]) {
    const app = workspace();
    app.elements.get(id).hidden = false;
    const paste = app.event(app.document.querySelector("main"), "paste", {
      clipboardData: { getData: () => "123" },
    });
    assert.equal(paste.defaultPrevented, false);
    assert.equal(app.expression(), "0");
    assert.equal(app.calls.length, 0);
  }
});

test("window state is never queried or changed without its independent permission", async () => {
  const app = workspace({ mode: "standard" });
  app.event(app.elements.get("always-on-top"), "click");
  await settle();
  assert.equal(app.elements.get("always-on-top").disabled, true);
  assert.equal(app.calls.filter((call) => call.method === "service").length, 0);
});

test("late window-state queries cannot restore controls after revocation hiding closing or session replacement", async () => {
  for (const change of ["revoke", "hide", "close", "import"]) {
    const pending = [];
    const app = workspace({
      mode: "standard",
      permissions: ["window:control"],
      serviceRequest: () => new Promise((resolve) => pending.push(resolve)),
    });
    const pin = app.elements.get("always-on-top");
    assert.equal(pending.length, 1);
    if (change === "revoke")
      app.onChange({ ...app.snapshot, grantedPermissions: [] });
    else if (change === "hide")
      app.onChange({ ...app.snapshot, active: false });
    else if (change === "close") app.windowEvent("pagehide");
    else
      app.onChange({
        ...app.snapshot,
        checkpoint: {
          schema: 1,
          fields: app.snapshot.fields,
          calculator: { mode: "standard", notation: true },
        },
      });
    pending[0]({ alwaysOnTop: true });
    await settle();
    assert.notEqual(pin.getAttribute("aria-pressed"), "true");
    assert.equal(pin.disabled, true);
    app.event(pin, "click");
    assert.equal(
      app.calls.filter(
        (call) =>
          call.method === "service" &&
          call.payload.action === "set-always-on-top",
      ).length,
      0,
    );
    if (change === "import") {
      assert.equal(pending.length, 2);
      pending[1]({ alwaysOnTop: false });
      await settle();
      assert.equal(pin.disabled, false);
    }
  }
});

test("window pin updates serialize clicks and remain retryable after a failed service receipt", async () => {
  const pending = [];
  const app = workspace({
    mode: "standard",
    permissions: ["window:control"],
    serviceRequest: (_service, payload) =>
      payload.action === "get-state"
        ? { alwaysOnTop: false }
        : new Promise((resolve, reject) => pending.push({ resolve, reject })),
  });
  await settle();
  const pin = app.elements.get("always-on-top");
  assert.equal(pin.disabled, false);
  pin.click();
  pin.click();
  app.event(pin, "click");
  assert.equal(pending.length, 1);
  assert.equal(pin.disabled, true);
  assert.equal(pin.dataset.locked, "true");
  pending[0].reject(new Error("Service unavailable"));
  await settle();
  assert.equal(pin.disabled, false);
  assert.equal(pin.getAttribute("aria-pressed"), "false");
  pin.click();
  assert.equal(pending.length, 2);
  pending[1].resolve({ alwaysOnTop: true });
  await settle();
  assert.equal(pin.disabled, false);
  assert.equal(pin.getAttribute("aria-pressed"), "true");
  pin.click();
  assert.equal(pending.length, 3);
  app.onChange({ ...app.snapshot, grantedPermissions: [] });
  assert.equal(pin.disabled, true);
  pending[2].resolve({ alwaysOnTop: false });
  await settle();
  assert.equal(pin.disabled, true);
  assert.equal(pin.getAttribute("aria-pressed"), "false");
});

test("currency automatic refresh requires permission for missing or expired cached rates", async () => {
  for (const cache of [
    { updatedAt: null, nextUpdateAt: null, result: null },
    { updatedAt: 1, nextUpdateAt: 2, result: "1.8" },
  ]) {
    for (const granted of [false, true]) {
      const data = converterOutput({
        category: "currency",
        from: "USD",
        to: "EUR",
        units: [{ id: "USD" }, { id: "EUR" }],
        ...cache,
      });
      const app = converterWorkspace(data, {
        permissions: granted ? ["network:https"] : [],
      });
      app.pressConverter("3");
      await app.flushTimers();
      const actions = app.calls
        .filter((call) => call.method === "run")
        .map((call) => call.action);
      assert.equal(actions.includes("converter-convert"), true);
      assert.equal(actions.includes("currency-refresh"), granted);
    }
  }
});

test("revoking currency network access before queued input runs suppresses automatic refresh", async () => {
  const app = converterWorkspace(
    converterOutput({
      category: "currency",
      from: "USD",
      to: "EUR",
      result: null,
      units: [{ id: "USD" }, { id: "EUR" }],
    }),
    { permissions: ["network:https"] },
  );
  app.pressConverter("3");
  app.onChange({ ...app.snapshot, grantedPermissions: [] });
  await app.flushTimers();
  assert.deepEqual(
    app.calls
      .filter((call) => call.method === "run")
      .map((call) => call.action),
    ["converter-convert"],
  );
  app.elements.get("currency-refresh").click();
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).action,
    "currency-refresh",
  );
});

test("an acknowledged draft freezes calculator objects before awaiting field synchronization", async () => {
  let finish;
  let requests = 0;
  const app = workspace({
    mode: "standard",
    setFieldsRequest: () => {
      requests++;
      if (requests === 1)
        return new Promise((resolve) => {
          finish = resolve;
        });
    },
  });
  app.press("2");
  await app.flushTimers();
  assert.equal(requests, 1);
  app.press("3");
  finish();
  await settle();
  const checkpoint = app.calls.find(
    (call) => call.method === "checkpoint",
  ).value;
  assert.equal(checkpoint.fields.expression, "2");
  assert.equal(checkpoint.calculator.standard.entry, "2");
  assert.equal(app.expression(), "23");
  app.onChange({
    ...app.snapshot,
    fields: checkpoint.fields,
    checkpoint,
  });
  assert.equal(app.expression(), "23");
  await app.flushTimers();
  const latest = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1).value;
  assert.equal(latest.fields.expression, "23");
  assert.equal(latest.calculator.standard.entry, "23");
});

test("long converter results are reused exactly through scientific notation and can reverse again", async () => {
  const app = converterWorkspace(
    converterOutput({ input: "1e300", result: "1" + "0".repeat(302) }),
  );
  app.elements.get("converter-value-to").click();
  assert.equal(app.elements.get("converterInput").value, "1e302");
  assert.equal(app.elements.get("converterFrom").value, "30");
  assert.equal(app.elements.get("converterTo").value, "34");
  const result = converterOutput({
    from: "30",
    to: "34",
    input: "1e302",
    result: "1e300",
  });
  app.responses.push(converterReceipt(app, result));
  await app.flushTimers();
  const request = app.calls.filter((call) => call.method === "run").at(-1);
  assert.equal(request.fields.converterInput, "1e302");
  assert.equal(app.elements.get("converter-output-from").textContent, "1e300");
  app.elements.get("converter-value-from").click();
  assert.equal(app.elements.get("converterInput").value, "1e300");
  assert.equal(app.elements.get("converterFrom").value, "34");
  assert.equal(app.elements.get("converterTo").value, "30");
});

test("converter result compaction preserves negative and tiny decimal values without rounding", () => {
  for (const [value, expected] of [
    ["-1" + "0".repeat(300), "-1e300"],
    ["-0." + "0".repeat(300) + "1250", "-1.25e-301"],
    ["1234500" + "0".repeat(300), "1.2345e306"],
  ]) {
    const app = converterWorkspace(converterOutput({ result: value }));
    app.elements.get("converter-value-to").click();
    assert.equal(app.elements.get("converterInput").value, expected);
  }
});

test("a converter result that cannot fit losslessly keeps the previous units input and side", () => {
  const app = converterWorkspace(
    converterOutput({ result: "1" + "2".repeat(300) }),
  );
  app.elements.get("converter-value-to").click();
  assert.equal(app.elements.get("converterInput").value, "2");
  assert.equal(app.elements.get("converterFrom").value, "34");
  assert.equal(app.elements.get("converterTo").value, "30");
  assert.equal(
    app.elements.get("converter-value-from").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(
    app.calls.filter(
      (call) => call.method === "checkpoint" || call.method === "run",
    ).length,
    0,
  );
  assert.equal(
    app.calls.find((call) => call.method === "notify").message,
    "This result is too long to reuse as input. Your current input was kept.",
  );
});

test("editing pasted fractions and scientific converter values never creates an incomplete numeric token", async () => {
  for (const [pasted, key, expected] of [
    [".5", "backspace", "0."],
    ["-.5", "backspace", "-0."],
    ["1e3", "7", "7"],
    ["1e3", ".", "0."],
    ["1e3", "backspace", "0"],
    ["1e3", "negate", "-1e3"],
    ["-1e-300", "negate", "1e-300"],
  ]) {
    const app = converterWorkspace(converterOutput({ supportsNegative: true }));
    app.event(app.document.querySelector("main"), "paste", {
      clipboardData: { getData: () => pasted },
    });
    app.pressConverter(key);
    assert.equal(app.elements.get("converterInput").value, expected);
    assert.match(
      app.elements.get("converterInput").value,
      /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/,
    );
    await app.flushTimers();
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields
        .converterInput,
      expected,
    );
  }
  const app = converterWorkspace(
    converterOutput({ result: "1" + "0".repeat(302) }),
  );
  app.elements.get("converter-value-to").click();
  app.pressConverter("backspace");
  assert.equal(app.elements.get("converterInput").value, "0");
});

test("malformed calculator substructures fall back to recoverable fields without throwing", async () => {
  for (const calculator of [
    {
      mode: "standard",
      notation: true,
      standard: { entry: "8", tokens: [], error: 1 },
    },
    {
      mode: "standard",
      standard: {
        entry: "8",
        tokens: [{ toString: null }],
        pending: "+",
      },
    },
    {
      mode: "scientific",
      justEvaluated: true,
      repeatOperation: { operand: "2", operator: { toString: null } },
    },
  ]) {
    const app = workspace({
      mode: calculator.mode,
      fields: { expression: "8" },
      state: { result: "8", ans: 8 },
      checkpoint: {
        schema: 1,
        fields: { mode: calculator.mode, expression: "8" },
        calculator,
      },
    });
    app.press("equals");
    await settle();
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields
        .expression,
      "8",
    );
  }
});

test("malformed nested mode state cannot crash switching or inject a repeat operation", async () => {
  const app = workspace({
    checkpoint: {
      schema: 1,
      fields: { mode: "graph", expression: "0" },
      calculator: {
        mode: "graph",
        notation: true,
        modeSessions: {
          standard: {
            expression: "44",
            standard: { entry: "44", tokens: [], error: {} },
          },
          scientific: {
            expression: "8",
            displayValue: "8",
            justEvaluated: true,
            repeatOperation: { operand: "2", operator: { toString: null } },
          },
        },
      },
    },
  });
  app.elements.get("standard-mode").click();
  assert.equal(app.expression(), "44");
  assert.equal(app.display(), "4.4e+1");
  app.elements.get("scientific-mode").click();
  app.press("equals");
  await settle();
  assert.equal(
    app.calls.filter((call) => call.method === "run").at(-1).fields.expression,
    "8",
  );
});

test("array converter sessions do not block length conversion or draft persistence", async () => {
  const app = workspace({
    mode: "converter",
    checkpoint: {
      schema: 1,
      fields: {
        mode: "converter",
        expression: "0",
        converterCategory: "length",
        converterInput: "0",
      },
      calculator: { mode: "converter", converterSessions: [] },
    },
  });
  app.pressConverter("7");
  await app.flushTimers();
  const request = app.calls.find((call) => call.method === "run");
  assert.equal(request.action, "converter-convert");
  assert.equal(request.fields.converterInput, "7");
  const saved = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1).value;
  assert.equal(Array.isArray(saved.calculator.converterSessions), false);
  assert.equal(saved.calculator.converterSessions.length.input, "7");
  assert.equal(
    app.calls.some((call) => call.method === "notify"),
    false,
  );
});

test("array mode sessions are replaced with serializable independent drafts", async () => {
  const app = workspace({
    checkpoint: {
      schema: 1,
      fields: { mode: "scientific", expression: "0" },
      calculator: { mode: "scientific", modeSessions: [] },
    },
  });
  app.press("7");
  app.elements.get("standard-mode").click();
  await app.flushTimers();
  const saved = JSON.parse(
    JSON.stringify(
      app.calls.filter((call) => call.method === "checkpoint").at(-1).value,
    ),
  );
  assert.equal(Array.isArray(saved.calculator.modeSessions), false);
  assert.equal(saved.calculator.modeSessions.scientific.expression, "7");
  const restored = workspace({ mode: "standard", checkpoint: saved });
  restored.elements.get("scientific-mode").click();
  assert.equal(restored.expression(), "7");
});

test("session replacement without a standard draft clears prior operations and mode inputs", async () => {
  for (const calculator of [undefined, { mode: "standard" }]) {
    const app = workspace({ mode: "standard" });
    app.press("7", "add", "2");
    app.elements.get("scientific-mode").click();
    app.press("9");
    app.elements.get("standard-mode").click();
    const fields = {
      ...app.snapshot.fields,
      mode: "standard",
      expression: "44",
    };
    const state = {
      ...JSON.parse(app.snapshot.view.state),
      fields,
      result: "44",
      ans: 44,
      ans_exact: "44",
    };
    app.onChange({
      ...app.snapshot,
      fields,
      view: { ...app.snapshot.view, state: JSON.stringify(state) },
      checkpoint: {
        schema: 1,
        fields,
        ...(calculator ? { calculator } : {}),
      },
    });
    assert.equal(app.expression(), "44");
    assert.equal(app.display(), "44");
    app.press("equals");
    await settle();
    assert.equal(
      app.calls.filter((call) => call.method === "run").at(-1).fields
        .expression,
      "44",
    );
    app.elements.get("scientific-mode").click();
    assert.equal(app.expression(), "0");
  }
});

test("valid pending standard operands and repeat equals survive checkpoint restoration", async () => {
  const app = workspace({ mode: "standard" });
  app.press("7", "add", "2");
  await app.flushTimers();
  const saved = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1).value;
  const restored = workspace({ mode: "standard", checkpoint: saved });
  restored.responses.push({ state: { result: "9", ans: 9 } });
  restored.press("equals");
  await settle();
  assert.equal(
    restored.calls.filter((call) => call.method === "run").at(-1).fields
      .expression,
    "7+2",
  );
  restored.press("equals");
  await settle();
  assert.equal(
    restored.calls.filter((call) => call.method === "run").at(-1).fields
      .expression,
    "(7+2)+(2)",
  );
});

test("graph flyout functions edit the active equation selection without evaluating science", async () => {
  for (const [attribute, operation, expected] of [
    ["data-trig", "sin", "sin(2*x)"],
    ["data-function", "abs", "abs(2*x)"],
    ["data-function", "floor", "floor(2*x)"],
    ["data-function", "ceil", "ceil(2*x)"],
    ["data-function", "dms", "dms(2*x)"],
    ["data-function", "degrees", "degrees(2*x)"],
  ]) {
    const app = workspace({
      mode: "graph",
      fields: { functions: "x\n2*x", expression: "123" },
    });
    const input = app.document.querySelector('[data-equation="1"]');
    input.focus();
    input.setSelectionRange(0, input.value.length);
    app.document.querySelector(`[${attribute}="${operation}"]`).click();
    await settle();
    assert.equal(input.value, expected);
    assert.equal(app.elements.get("functions").value, "x\n" + expected);
    assert.equal(app.expression(), "123");
    assert.equal(app.document.activeElement, input);
    assert.equal(
      app.calls.some((call) => call.method === "run"),
      false,
    );
    await app.flushTimers();
    const checkpoint = app.calls
      .filter((call) => call.method === "checkpoint")
      .at(-1);
    assert.equal(checkpoint.value.fields.functions, "x\n" + expected);
  }
});

test("graph flyouts insert inverse hyperbolic functions and random values at the equation caret", async () => {
  const app = workspace({
    mode: "graph",
    fields: { functions: "x+", expression: "123" },
  });
  const input = app.document.querySelector('[data-equation="0"]');
  input.focus();
  input.setSelectionRange(2, 2);
  app.elements.get("trig-second").click();
  app.elements.get("trig-hyp").click();
  app.document.querySelector('[data-trig="sin"]').click();
  assert.equal(input.value, "x+asinh(");
  assert.equal(input.selectionStart, input.value.length);
  assert.equal(
    app.elements.get("trig-second").getAttribute("aria-pressed"),
    "false",
  );
  assert.equal(
    app.elements.get("trig-hyp").getAttribute("aria-pressed"),
    "false",
  );
  input.value = "x+";
  input.setSelectionRange(2, 2);
  app.document.querySelector('[data-function="rand"]').click();
  assert.equal(input.value.startsWith("x+"), true);
  const random = Number(input.value.slice(2));
  assert.equal(Number.isFinite(random) && random >= 0 && random < 1, true);
  assert.equal(app.elements.get("functions").value, input.value);
  assert.equal(app.expression(), "123");
  assert.equal(
    app.calls.some((call) => call.method === "run"),
    false,
  );
});

test("calendar month changes retain a single keyboard entry and arrow navigation moves it", () => {
  const app = workspace({ mode: "date", fields: { dateFrom: "2024-02-15" } });
  app.elements.get("date-from-button").click();
  const grid = app.elements.get("calendar-grid"),
    focusable = () =>
      grid.querySelectorAll("button").filter((node) => node.tabIndex === 0);
  assert.deepEqual(
    focusable().map((node) => node.dataset.date),
    ["2024-02-15"],
  );
  const month = app.elements.get("calendar-month");
  month.value = "6";
  app.event(month, "change");
  assert.deepEqual(
    focusable().map((node) => node.dataset.date),
    ["2024-06-01"],
  );
  focusable()[0].focus();
  app.key("ArrowRight", focusable()[0]);
  assert.deepEqual(
    focusable().map((node) => node.dataset.date),
    ["2024-06-02"],
  );
  assert.equal(app.document.activeElement, focusable()[0]);
  assert.equal(app.elements.get("date-from").value, "2024-02-15");
});

test("calendar year changes keep the keyboard entry inside the supported date range", () => {
  const app = workspace({ mode: "date", fields: { dateFrom: "2024-02-15" } });
  app.elements.get("date-from-button").click();
  const month = app.elements.get("calendar-month"),
    year = app.elements.get("calendar-year"),
    grid = app.elements.get("calendar-grid");
  for (const [newYear, newMonth, expected] of [
    ["1601", "1", "1601-01-01"],
    ["9999", "12", "9999-12-01"],
  ]) {
    month.value = newMonth;
    app.event(month, "change");
    year.value = newYear;
    app.event(year, "change");
    const buttons = grid.querySelectorAll("button"),
      focusable = buttons.filter((node) => node.tabIndex === 0);
    assert.equal(focusable.length, 1);
    assert.equal(focusable[0].dataset.date, expected);
    assert.equal(focusable[0].disabled, false);
    for (const button of buttons)
      if (
        button.dataset.date < "1601-01-01" ||
        button.dataset.date > "9999-12-31"
      ) {
        assert.equal(button.disabled, true);
        assert.equal(button.tabIndex, -1);
      }
  }
});

test("graph canvas paste edits the active equation while equation inputs retain native paste", async () => {
  const app = workspace({
    mode: "graph",
    fields: { functions: "x\n2*x", expression: "123" },
  });
  const input = app.document.querySelector('[data-equation="1"]'),
    canvas = app.elements.get("graph");
  input.focus();
  input.setSelectionRange(0, input.value.length);
  canvas.focus();
  const pasted = app.event(canvas, "paste", {
    clipboardData: { getData: () => "log(x,2)" },
  });
  assert.equal(pasted.defaultPrevented, true);
  assert.equal(input.value, "log(x,2)");
  assert.equal(app.elements.get("functions").value, "x\nlog(x,2)");
  assert.equal(app.expression(), "123");
  assert.equal(
    app.calls.some((call) => call.method === "run"),
    false,
  );
  await app.flushTimers();
  const checkpoint = app.calls
    .filter((call) => call.method === "checkpoint")
    .at(-1);
  assert.equal(checkpoint.value.fields.functions, "x\nlog(x,2)");
  const nativePaste = app.event(input, "paste", {
    clipboardData: { getData: () => "cos(x)" },
  });
  assert.equal(nativePaste.defaultPrevented, false);
  assert.equal(input.value, "log(x,2)");
  assert.equal(app.expression(), "123");
});

test("graph selections survive a deferred input rebuild after focus moves to a flyout", () => {
  for (const [start, end, expected] of [
    [0, 1, "sin(x)"],
    [1, 1, "xsin("],
  ]) {
    const app = workspace({ mode: "graph", fields: { functions: "x" } });
    const original = app.document.querySelector('[data-equation="0"]'),
      trigger = app.elements.get("trig-menu");
    original.focus();
    app.event(original, "input");
    original.setSelectionRange(start, end);
    app.onChange(app.snapshot);
    assert.equal(app.document.querySelector('[data-equation="0"]'), original);
    trigger.focus();
    trigger.click();
    app.onChange(app.snapshot);
    const rebuilt = app.document.querySelector('[data-equation="0"]');
    assert.notEqual(rebuilt, original);
    assert.equal(rebuilt.selectionStart, start);
    assert.equal(rebuilt.selectionEnd, end);
    assert.equal(app.document.activeElement, trigger);
    app.document.querySelector('[data-trig="sin"]').click();
    assert.equal(rebuilt.value, expected);
    assert.equal(app.elements.get("functions").value, expected);
  }
});

test("graph input rebuilds do not restore a selection onto different equation text", () => {
  const app = workspace({ mode: "graph", fields: { functions: "x" } });
  const original = app.document.querySelector('[data-equation="0"]'),
    trigger = app.elements.get("trig-menu");
  original.focus();
  app.event(original, "input");
  original.setSelectionRange(0, 1);
  app.elements.get("functions").value = "y+1";
  trigger.focus();
  app.onChange(app.snapshot);
  const rebuilt = app.document.querySelector('[data-equation="0"]');
  assert.equal(rebuilt.value, "y+1");
  assert.equal(rebuilt.selectionStart, undefined);
  assert.equal(rebuilt.selectionEnd, undefined);
  assert.equal(app.document.activeElement, trigger);
});

test("programmer shift controls expose the restored choice as native toggle buttons", async () => {
  const app = workspace({
    mode: "programmer",
    checkpoint: {
      schema: 1,
      fields: { mode: "programmer", expression: "0" },
      calculator: { mode: "programmer", shiftKind: "logical" },
    },
  });
  const popup = app.elements.get("shift-popup");
  assert.equal(popup.getAttribute("role"), "group");
  assert.equal(popup.getAttribute("aria-labelledby"), "shift-menu");
  const buttons = app.document.querySelectorAll("[data-shift]");
  for (const button of buttons) {
    assert.equal(button.getAttribute("role"), null);
    assert.equal(button.getAttribute("aria-checked"), null);
    assert.equal(
      button.getAttribute("aria-pressed"),
      String(button.dataset.shift === "logical"),
    );
  }
  app.elements.get("shift-menu").click();
  buttons.find((button) => button.dataset.shift === "rotate").click();
  assert.equal(popup.hidden, true);
  for (const button of buttons)
    assert.equal(
      button.getAttribute("aria-pressed"),
      String(button.dataset.shift === "rotate"),
    );
  await app.flushTimers();
  const saved = app.calls.filter((call) => call.method === "checkpoint").at(-1);
  const restored = workspace({ mode: "programmer", checkpoint: saved.value });
  assert.equal(
    restored.document
      .querySelector('[data-shift="rotate"]')
      .getAttribute("aria-pressed"),
    "true",
  );
});
