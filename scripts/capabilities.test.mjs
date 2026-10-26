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
