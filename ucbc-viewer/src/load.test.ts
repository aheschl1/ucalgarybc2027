import { describe, expect, it } from "vitest";

import { parseReplay } from "./load.ts";

function stream(text: string, chunk: number): ReadableStream<Uint8Array> {
  const bytes = new TextEncoder().encode(text);
  return new ReadableStream({
    start(c) {
      for (let i = 0; i < bytes.length; i += chunk) c.enqueue(bytes.subarray(i, i + chunk));
      c.close();
    },
  });
}

const replay = {
  config: { game: "g", ticks: [1] },
  sets: [
    { initial_state: { ticks: [] }, ticks: [{ number: 1, steps: [{ say: 'a "}]" \\ é' }] }, { number: 2 }] },
    { ticks: [] },
    { ticks: [{ number: 1, state_after: [[{}]] }] },
  ],
  result: null,
};

describe("parseReplay", () => {
  it("matches JSON.parse however the bytes are split", async () => {
    for (const chunk of [1, 2, 3, 5, 1000]) {
      expect(await parseReplay(stream(JSON.stringify(replay, null, 1), chunk))).toEqual(replay);
    }
  });

  it("agrees with JSON.parse on odd and broken input", async () => {
    const cases = [
      '{"sets": [{"ticks": [{]}]}',
      "<html>",
      '{"sets":[{"ticks":[{}{}]}]}',
      '{"sets":[{"ticks":[{} xyz {}]}]}',
      '{"sets":[{"ticks":[{},]}]}',
      '{"sets":[{"ticks":[,{}]}]}',
      '{"sets":[{"ticks":[ ]}]}',
      '{"sets":[{"ticks":[1, "a", {"n":1}]}]}',
      '{"config":{"game_config":{"ticks":[{"a":1}]}},"sets":[{"ticks":[{"n":2}]}]}',
      '{"sets":[{"x":1},2,{"ticks" : [{"n":2}]}]}',
      '{"sets":[{"ti\\"cks":[{"n":1}]}]}',
      '{"sets":[["ticks",[{"n":1}]]]}',
      '{"sets":[{"ticks":[{"n":1}],"ticks":[{"n":2}]}]}',
      '{"sets":[{"ticks":[{"n":1}',
    ];
    for (const text of cases) {
      let want: unknown;
      try {
        want = JSON.parse(text);
      } catch {
        await expect(parseReplay(stream(text, 3)), text).rejects.toThrow(SyntaxError);
        continue;
      }
      expect(await parseReplay(stream(text, 3)), text).toEqual(want);
    }
  });
});
