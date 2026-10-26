import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const html = readFileSync(new URL("./main.html", import.meta.url), "utf8");
const source = html.match(/<script>([\s\S]*?)<\/script>/)[1];

function workspace() {
  const elements = new Map();
  const nodes = [];
  const document = { activeElement: null };
  class Element {
    constructor(tagName = "div") {
      this.tagName = tagName.toUpperCase();
      this.dataset = {};
      this.classList = { contains: () => false };
      this.style = {};
      this.children = [];
      this.value = "";
      this.textContent = "";
      this.hidden = false;
      this.attributes = new Map();
      nodes.push(this);
    }
    setAttribute(key, value) {
      this.attributes.set(key, value);
    }
    removeAttribute(key) {
      this.attributes.delete(key);
    }
    append(...children) {
      this.children.push(...children);
    }
    replaceChildren(...children) {
      this.children = children;
    }
    addEventListener() {}
    focus() {
      document.activeElement = this;
    }
    setSelectionRange(start, end) {
      this.selectionStart = start;
      this.selectionEnd = end;
    }
    getContext() {
      return null;
    }
    querySelectorAll(selector) {
      return this.children.flatMap((child) =>
        child instanceof Element
          ? [
              ...(selector === "button" && child.tagName === "BUTTON"
                ? [child]
                : []),
              ...child.querySelectorAll(selector),
            ]
          : [],
      );
    }
  }
  for (const [, tag, id] of html.matchAll(/<([a-z]+)\b[^>]*\bid="([^"]+)"/g)) {
    elements.set(id, new Element(tag));
  }
  const main = new Element("main");
  const root = new Element("html");
  const memoryActions = new Map(
    ["memory-store", "memory-add", "memory-clear"].map((id) => [
      id,
      new Element("button"),
    ]),
  );
  for (const [id, button] of memoryActions) button.dataset.action = id;
  Object.assign(document, {
    documentElement: root,
    getElementById: (id) => elements.get(id),
    createElement: (tag) => new Element(tag),
    createTextNode: (text) => ({ textContent: text }),
    addEventListener() {},
    querySelector: (selector) =>
      selector === "main"
        ? main
        : memoryActions.get(selector.match(/data-action="([^"]+)"/)?.[1]),
    querySelectorAll: (selector) => {
      if (selector === "button")
        return nodes.filter((node) => node.tagName === "BUTTON");
      if (selector === "[data-action]") return [...memoryActions.values()];
      if (selector === "[data-special]")
        return nodes.filter((node) => node.dataset.special);
      return [];
    },
  });
  let onChange;
  const starter = {
    onChange: (callback) => {
      onChange = callback;
    },
    notify: async () => {},
    setFields: async () => {},
    checkpoint: async () => {},
  };
  vm.runInNewContext(source, {
    document,
    starter,
    window: { devicePixelRatio: 1, addEventListener() {} },
    ResizeObserver: class {
      observe() {}
      disconnect() {}
    },
    matchMedia: () => ({ addEventListener() {} }),
    requestAnimationFrame: () => 1,
    cancelAnimationFrame() {},
    setTimeout: () => 1,
    clearTimeout() {},
    performance: { now: () => 0 },
  });
  const fields = {
    expression: "1+1",
    angle: "rad",
    functions: "x",
    xmin: "-10",
    xmax: "10",
    ymin: "-3",
    ymax: "3",
    a: "0",
    b: "1",
    curve: "0",
  };
  const snapshot = {
    locale: "en",
    busy: false,
    grantedPermissions: [],
    fields,
    checkpoint: null,
    view: {
      state: JSON.stringify({
        schema: 1,
        result: "2",
        memory: 0,
        fields,
        history: [{ expression: "1+1", value: 2, angle: "rad" }],
        plot: { series: [], xmin: -10, xmax: 10, ymin: -3, ymax: 3 },
      }),
      errors: {},
      artifacts: [],
      groups: [
        {
          actions: [...memoryActions.keys()].map((id) => ({
            id,
            disabled: false,
          })),
        },
      ],
    },
  };
  onChange(snapshot);
  return { elements, document, snapshot, onChange };
}

test("backspace deletes a complete Unicode code point", () => {
  const { elements } = workspace();
  const input = elements.get("expression");
  input.value = "2+😀";
  input.setSelectionRange(input.value.length, input.value.length);
  const backspace = elements
    .get("keypad")
    .children.find((button) => button.dataset.special === "backspace");
  backspace.onclick();
  assert.equal(input.value, "2+");
  assert.equal(input.selectionStart, 2);
});

test("unchanged host snapshots preserve focused history elements", () => {
  const { elements, document, snapshot, onChange } = workspace();
  const row = elements.get("history").children[0].children[0];
  row.focus();
  onChange({ ...snapshot });
  assert.equal(elements.get("history").children[0].children[0], row);
  assert.equal(document.activeElement, row);
});

test("locale changes update the document language for assistive technology", () => {
  const { document, snapshot, onChange } = workspace();
  assert.equal(document.documentElement.lang, "en");
  onChange({ ...snapshot, locale: "zh-CN" });
  assert.equal(document.documentElement.lang, "zh-CN");
});
