import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Execute the actual shipped engine without a canvas or a second rules model.
const html = readFileSync(new URL("./main.html", import.meta.url), "utf8");
const engine = html.match(
  /\/\/ SLITHER_ENGINE_BEGIN([\s\S]*?)\/\/ SLITHER_ENGINE_END/,
);
assert.ok(engine, "the custom UI must contain the deterministic arena engine");
// Use this realm's numeric intrinsics: VM context proxies make a ten-minute
// replay about 50 times slower and do not provide a security boundary here.
const E = new Function(
  "globalThis",
  `${engine[1]}; return globalThis.SlitherEngine;`,
)({});
const plain = (value) => JSON.parse(JSON.stringify(value));

function fingerprint(world) {
  let hash = 2166136261;
  const number = (value) => {
    value >>>= 0;
    for (let byte = 0; byte < 4; byte++) {
      hash = Math.imul(hash ^ (value & 255), 16777619) >>> 0;
      value >>>= 8;
    }
  };
  [world.tick, world.rng, world.nextFoodId].forEach(number);
  for (const snake of world.snakes) {
    [
      snake.id,
      +snake.alive,
      snake.x,
      snake.y,
      snake.angle,
      snake.targetAngle,
      snake.score,
      snake.bestScore,
      snake.kills,
      snake.foods,
      +snake.boosting,
      snake.radius,
      snake.length,
      snake.turn,
      snake.respawnAt,
      snake.body.length,
    ].forEach(number);
    for (const point of snake.body) point.forEach(number);
  }
  number(world.foods.length);
  for (const item of world.foods)
    [item.id, item.x, item.y, item.value, item.color].forEach(number);
  return hash.toString(16).padStart(8, "0");
}

test("continuous headings turn toward a diagonal and preserve the original angular speed limit", () => {
  const world = E.create();
  E.step(world, { angle: 13, boost: false });
  assert.equal(world.snakes[0].angle, 12);
  assert.ok(world.snakes[0].x > 0 && world.snakes[0].y > 0);
  E.step(world, { angle: 13, boost: false });
  assert.equal(world.snakes[0].angle, 13);
  assert.equal(world.snakes.length, 13);
});

test("eating, boosting, AI state and RNG match the independent Rust replay", () => {
  const world = E.replay(
    E.initial({
      inputs: [
        [13, false, 8],
        [81, false, 15],
        [130, true, 12],
      ],
    }),
  );
  assert.deepEqual(plain(E.summary(world)), {
    score: 10,
    maxScore: 13,
    kills: 0,
    foods: 1,
    ticks: 35,
    status: "running",
  });
  assert.equal(fingerprint(world), "488d3971");
});

test("bot deaths, loot and respawning preserve full-world Rust parity", () => {
  const inputs = Array.from({ length: 1200 }, (_, tick) => [
    (tick * 3) & 255,
    tick % 100 < 20,
    1,
  ]);
  const world = E.replay(
    E.initial({ seed: 0x12345678, difficulty: "hard", inputs }),
  );
  assert.equal(fingerprint(world), "fa22d378");
  assert.ok(world.foods.length > E.constants.FOOD_COUNT);
  assert.ok(world.foods.length <= E.constants.MAX_FOOD);
});

test("the ten minute round remains bounded and matches the Rust final world", () => {
  const inputs = Array.from({ length: 1200 }, (_, segment) => [
    (segment * 10) & 255,
    false,
    10,
  ]);
  const world = E.replay(E.initial({ seed: 1, difficulty: "easy", inputs }));
  assert.deepEqual(plain(E.summary(world)), {
    score: 83,
    maxScore: 83,
    kills: 0,
    foods: 35,
    ticks: 12000,
    status: "finished",
  });
  assert.equal(fingerprint(world), "a6425d75");
  assert.ok(
    world.snakes.every((snake) => snake.body.length <= E.constants.MAX_BODY),
  );
  assert.throws(() => E.step(world, { angle: 0, boost: false }));
});

test("the circular world boundary kills the player and rejects input after death", () => {
  const checkpoint = E.initial({
    seed: 9,
    difficulty: "easy",
    inputs: [[0, false, 426]],
  });
  const world = E.replay(checkpoint);
  assert.equal(world.status, "dead");
  assert.equal(fingerprint(world), "7715faab");
  assert.throws(() => E.replay({ ...checkpoint, inputs: [[0, false, 427]] }));
});

test("self crossing is permitted while another snake body is lethal", () => {
  const world = E.create(12, "easy");
  for (const snake of world.snakes.slice(1)) {
    snake.alive = false;
    snake.respawnAt = 12001;
  }
  world.snakes[0].body = Array.from({ length: 20 }, () => [240, 0]);
  E.step(world, { angle: 0, boost: false });
  assert.equal(world.snakes[0].alive, true);
  const other = world.snakes[1];
  other.alive = true;
  other.x = 8000;
  other.y = 8000;
  other.body = Array.from({ length: 20 }, () => [480, 0]);
  E.step(world, { angle: 0, boost: false });
  assert.equal(world.status, "dead");
  assert.ok(world.foods.some((item) => item.kind === "loot"));
});

test("boost consumes mass, doubles travel, drops food and preserves peak score", () => {
  const world = E.create(8, "easy");
  world.snakes[0].score = 100;
  world.snakes[0].bestScore = 100;
  const count = world.foods.length;
  for (let tick = 0; tick < 3; tick++) E.step(world, { angle: 0, boost: true });
  assert.ok(world.snakes[0].score < 100);
  assert.equal(world.snakes[0].bestScore, 100);
  assert.ok(world.snakes[0].x >= 1400);
  assert.ok(world.foods.length > count);
  assert.ok(world.foods.some((item) => item.kind === "dropping"));
});

test("minimum mass cannot boost or become negative", () => {
  const world = E.create(8, "easy");
  E.step(world, { angle: 0, boost: true });
  assert.equal(world.snakes[0].boosting, false);
  assert.equal(world.snakes[0].x, 240);
  assert.ok(world.snakes[0].score >= 10);
});

test("input logs merge equal controls and do not alias restored checkpoints", () => {
  const checkpoint = E.initial();
  const restored = E.validateCheckpoint(checkpoint);
  E.appendInput(checkpoint, { angle: 37, boost: false });
  E.appendInput(checkpoint, { angle: 37, boost: false });
  E.appendInput(checkpoint, { angle: 37, boost: true });
  assert.deepEqual(plain(checkpoint.inputs), [
    [37, false, 2],
    [37, true, 1],
  ]);
  assert.deepEqual(plain(restored.inputs), []);
});

test("unknown future fields and invalid checkpoint values are preserved by rejection", () => {
  const invalid = [
    { schema: 4 },
    { seed: 0 },
    { seed: -1 },
    { seed: 4294967296 },
    { difficulty: "fast" },
    { skin: 15 },
    { muted: 1 },
    { volume: 101 },
    { nickname: "😀".repeat(25) },
    { nickname: "bad\u0000name" },
    { nickname: "\ud800" },
    { nickname: "\udc00" },
    { nickname: "a\ud800b" },
    { inputs: [[256, false, 1]] },
    { inputs: [[-1, false, 1]] },
    { inputs: [[3.5, false, 1]] },
    { inputs: [[0, 1, 1]] },
    { inputs: [[0, false, 0]] },
    { inputs: [[0, false, 12001]] },
    {
      inputs: [
        [0, false, 12000],
        [0, false, 1],
      ],
    },
    { inputs: [[0, false, 1, 2]] },
    { score: 999 },
  ];
  for (const mutation of invalid)
    assert.throws(
      () => E.validateCheckpoint({ ...E.initial(), ...mutation }),
      JSON.stringify(mutation),
    );
  assert.equal(
    E.validateCheckpoint(E.initial({ nickname: "😀".repeat(24) })).nickname
      .length,
    48,
  );
});

test("checkpoint and live append resource limits are consistent", () => {
  const turning = E.initial({ skin: 14 });
  for (let tick = 0; tick < E.constants.MAX_TICKS; tick++)
    E.appendInput(turning, { angle: tick & 255, boost: false });
  assert.equal(
    E.validateCheckpoint(turning).inputs.length,
    E.constants.MAX_TICKS,
  );
  assert.throws(() => E.appendInput(turning, { angle: 0, boost: false }));
  const tooMany = Array.from({ length: E.constants.MAX_INPUTS + 1 }, () => [
    0,
    false,
    1,
  ]);
  assert.throws(() => E.initial({ inputs: tooMany }));
  const full = E.initial({ inputs: [[0, false, 12000]] });
  assert.throws(() => E.appendInput(full, { angle: 0, boost: false }));
});
