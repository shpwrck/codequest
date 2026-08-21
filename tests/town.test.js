import test from "node:test";
import assert from "node:assert/strict";

await import("../src/town.js");
const { createTown, routeBetween, kindForFiles } = globalThis.CodeQuestTown;

test("filesystem groups become deterministic town landmarks", () => {
  const files = [
    { path: "README.md" },
    { path: "package.json" },
    { path: "src/main.js" },
    { path: "src/styles.css" },
    { path: "tests/town.test.js" },
    { path: "docs/guide.md" },
  ];
  const first = createTown("code-quest", files);
  const second = createTown("code-quest", [...files].reverse());

  assert.deepEqual(first, second);
  assert.deepEqual(first.landmarks.map((place) => place.id), ["$root", "src", "docs", "tests"]);
  assert.equal(first.landmarks[0].name, "CODE QUEST HALL");
  assert.equal(first.landmarks.find((place) => place.id === "src").kind, "code");
  assert.equal(first.landmarks.find((place) => place.id === "tests").kind, "tests");
});

test("towns cap landmarks to the available building plots", () => {
  const files = Array.from({ length: 14 }, (_, index) => ({ path: `district-${index}/file.js` }));
  const town = createTown("many-folders", files);

  assert.equal(town.landmarks.length, 10);
  assert.equal(new Set(town.landmarks.map((place) => `${place.x},${place.y}`)).size, 10);
});

test("routes use the main road and end at the requested quiz stop", () => {
  const town = createTown("route-test", [
    { path: "src/main.js" },
    { path: "tests/main.test.js" },
  ]);
  const from = town.start;
  const to = town.landmarks[1].door;
  const route = routeBetween(town, from, to);

  assert.deepEqual(route[0], from);
  assert.deepEqual(route.at(-1), to);
  assert.ok(route.every(({ x, y }) => x >= 0 && x < town.width && y >= 0 && y < town.height));
  assert.ok(route.some(({ y }) => y === town.roadY));
  for (let index = 1; index < route.length; index++) {
    const distance = Math.abs(route[index].x - route[index - 1].x) + Math.abs(route[index].y - route[index - 1].y);
    assert.equal(distance, 1);
  }
});

test("file contents select recognizable building types", () => {
  assert.equal(kindForFiles(["docs/guide.md"], "docs"), "docs");
  assert.equal(kindForFiles(["public/logo.svg"], "public"), "assets");
  assert.equal(kindForFiles(["src/lib.rs"], "src"), "code");
  assert.equal(kindForFiles(["test-utils/helpers.js"], "test-utils"), "tests");
  assert.equal(kindForFiles(["Cargo.toml"], "$root"), "config");
});

test("an empty repository still gets a town hall", () => {
  const town = createTown("empty-repository-with-a-long-name", []);
  assert.equal(town.landmarks.length, 1);
  assert.equal(town.landmarks[0].id, "$root");
  assert.equal(town.landmarks[0].fileCount, 0);
  assert.equal(town.name, "EMPTY TOWN");
  assert.equal(town.landmarks[0].name, "EMPTY HALL");
});
