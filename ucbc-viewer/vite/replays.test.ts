import { expect, it } from "vitest";

import { sampleReplays } from "./replays.ts";

it("finds each game's sample replays under games/", () => {
  const list = sampleReplays();
  expect(list).toContain("ucbc2027/standard.json.gz");
  expect(list).toEqual([...list].sort());
});
