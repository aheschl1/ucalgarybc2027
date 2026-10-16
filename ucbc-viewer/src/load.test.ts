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

  it("rejects what is not JSON", async () => {
    await expect(parseReplay(stream('{"sets": [{"ticks": [{]}]}', 4))).rejects.toThrow(SyntaxError);
    await expect(parseReplay(stream("<html>", 4))).rejects.toThrow(SyntaxError);
  });
});
