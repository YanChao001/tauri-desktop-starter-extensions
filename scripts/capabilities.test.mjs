import assert from "node:assert/strict";
import { test } from "node:test";
import { validateManifest } from "./protocol.mjs";

const manifest = () => ({
  formatVersion: 3,
  apiVersion: 2,
  minHostVersion: "0.1.6",
  id: "example.capabilities",
  version: "1.0.0",
  name: "Capability fixture",
  author: "Tests",
  description: "Test-only permission validation",
  commands: [
    {
      id: "main",
      title: "Main",
      function: "main",
      inputFormat: "text",
      resultFormat: "text",
      permissions: [],
    },
  ],
  permissions: [],
  optionalPermissions: [],
  network: [],
});

test("media and external-link capabilities require the host version that implements them", () => {
  for (const permission of [
    "network:images",
    "network:system-proxy",
    "links:open",
  ]) {
    for (const scope of ["permissions", "optionalPermissions"]) {
      const extension = manifest();
      extension[scope].push(permission);
      if (permission === "network:system-proxy")
        extension.optionalPermissions.push("network:images");
      for (const minHostVersion of ["0.0.65535", "0.1.4", "0.1.5"])
        assert.throws(
          () => validateManifest({ ...extension, minHostVersion }),
          /minHostVersion 0.1.6/,
        );
      for (const minHostVersion of ["0.1.6", "0.1.10", "0.2.0", "1.0.0"])
        validateManifest({ ...extension, minHostVersion });
    }
  }
  assert.throws(
    () =>
      validateManifest({
        ...manifest(),
        optionalPermissions: ["network:unknown"],
      }),
    /permissions/,
  );
});

test("system-proxy declaration requires public HTTPS origins or image access", () => {
  const extension = {
    ...manifest(),
    optionalPermissions: ["network:system-proxy"],
  };
  assert.throws(
    () => validateManifest(extension),
    /requires declared public HTTP or image access/,
  );
  assert.throws(
    () =>
      validateManifest({
        ...extension,
        optionalPermissions: [
          ...extension.optionalPermissions,
          "network:local",
        ],
        network: [{ origin: "http://localhost:11434", methods: ["POST"] }],
      }),
    /requires declared public HTTP or image access/,
  );
  validateManifest({
    ...extension,
    optionalPermissions: [...extension.optionalPermissions, "network:images"],
  });
  validateManifest({
    ...extension,
    optionalPermissions: [...extension.optionalPermissions, "network:https"],
    network: [{ origin: "https://api.example.com", methods: ["GET"] }],
  });
  assert.throws(
    () =>
      validateManifest({
        ...extension,
        optionalPermissions: [
          ...extension.optionalPermissions,
          "network:https",
        ],
      }),
    /does not match declared origins/,
  );
});

test("window control declarations require the host version that implements them", () => {
  for (const scope of ["permissions", "optionalPermissions"]) {
    const extension = manifest();
    extension[scope].push("window:control");
    for (const minHostVersion of ["0.0.65535", "0.1.4", "0.1.5", "0.1.6"])
      assert.throws(
        () => validateManifest({ ...extension, minHostVersion }),
        /Window control requires minHostVersion 0\.1\.7/,
      );
    for (const minHostVersion of ["0.1.7", "0.1.10", "0.2.0", "1.0.0"])
      validateManifest({ ...extension, minHostVersion });
    assert.throws(
      () =>
        validateManifest({
          ...extension,
          minHostVersion: "0.1.7",
          [scope === "permissions" ? "optionalPermissions" : "permissions"]: [
            "window:control",
          ],
        }),
      /Invalid required or optional permissions/,
    );
  }
});

test("text commands reject image and proxy network gates while optional declarations remain usable", () => {
  for (const permission of ["network:images", "network:system-proxy"]) {
    const extension = {
      ...manifest(),
      optionalPermissions: ["network:images", "network:system-proxy"],
    };
    validateManifest(extension);
    assert.throws(
      () =>
        validateManifest({
          ...extension,
          commands: [{ ...extension.commands[0], permissions: [permission] }],
        }),
      /command contribution or permissions/,
    );
    validateManifest({
      ...extension,
      commands: [
        {
          ...extension.commands[0],
          inputFormat: "json",
          permissions: [permission],
        },
      ],
    });
  }
  const extension = { ...manifest(), optionalPermissions: ["links:open"] };
  validateManifest({
    ...extension,
    commands: [{ ...extension.commands[0], permissions: ["links:open"] }],
  });
});

test("custom window sizes require bounded integer dimensions and the implementing host", () => {
  const extension = manifest();
  extension.minHostVersion = "0.1.7";
  const command = extension.commands[0];
  command.inputFormat = "json";
  command.resultFormat = "view";
  command.ui = {
    sidebar: true,
    window: true,
    windowSize: { width: 1100, height: 820 },
  };
  validateManifest(extension);
  for (const size of [
    { width: 360, height: 480 },
    { width: 1920, height: 1440 },
  ]) {
    command.ui.windowSize = size;
    validateManifest(extension);
  }
  for (const size of [
    { width: 359, height: 820 },
    { width: 1100, height: 479 },
    { width: 1921, height: 820 },
    { width: 1100, height: 1441 },
    { width: 1100.5, height: 820 },
    { width: "1100", height: 820 },
    { width: 1100 },
    { width: 1100, height: 820, url: "https://example.com" },
  ]) {
    command.ui.windowSize = size;
    assert.throws(() => validateManifest(extension), /window size/);
  }
  command.ui.windowSize = { width: 1100, height: 820 };
  command.ui.window = false;
  assert.throws(() => validateManifest(extension), /window size/);
  command.ui.window = true;
  for (const minHostVersion of ["0.0.65535", "0.1.6"])
    assert.throws(
      () => validateManifest({ ...extension, minHostVersion }),
      /minimum host/,
    );
  for (const minHostVersion of ["0.1.7", "0.1.10", "0.2.0", "1.0.0"])
    validateManifest({ ...extension, minHostVersion });
});

test("compute execution budgets require the implementing host without adding permissions", () => {
  const extension = manifest();
  const command = extension.commands[0];
  command.executionBudget = "compute";
  for (const minHostVersion of ["0.0.65535", "0.1.4", "0.1.5", "0.1.6"])
    assert.throws(
      () => validateManifest({ ...extension, minHostVersion }),
      /Compute execution budget requires minHostVersion 0\.1\.7/,
    );
  for (const minHostVersion of ["0.1.7", "0.1.10", "0.2.0", "1.0.0"])
    validateManifest({ ...extension, minHostVersion });

  for (const budget of [undefined, "standard"]) {
    if (budget === undefined) delete command.executionBudget;
    else command.executionBudget = budget;
    for (const minHostVersion of ["0.0.1", "0.1.4", "0.1.7"])
      validateManifest({ ...extension, minHostVersion });
  }
  assert.throws(
    () =>
      validateManifest({ ...extension, optionalPermissions: ["compute"] }),
    /Invalid required or optional permissions/,
  );
});

test("execution budget declarations reject null, unknown values and misplaced fields", () => {
  const extension = manifest();
  extension.minHostVersion = "0.1.7";
  const command = extension.commands[0];
  for (const executionBudget of [
    null,
    undefined,
    "",
    "unlimited",
    "Compute",
    2_000_000_000,
    false,
    [],
    {},
  ]) {
    command.executionBudget = executionBudget;
    assert.throws(
      () => validateManifest(extension),
      /Invalid command contribution or permissions/,
    );
  }
  delete command.executionBudget;
  assert.throws(
    () => validateManifest({ ...extension, executionBudget: "compute" }),
    /Invalid extension identity or API/,
  );
  assert.throws(
    () =>
      validateManifest({
        ...extension,
        commands: [{ ...command, executionBudgets: "compute" }],
      }),
    /Invalid command contribution or permissions/,
  );
});
