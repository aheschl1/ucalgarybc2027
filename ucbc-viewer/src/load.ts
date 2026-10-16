// Long matches make replays larger than the longest string a browser allows, so a replay
// is parsed as it streams in: each set's ticks one at a time, and the rest once at the end.

const OPEN = new Set([0x7b, 0x5b]); // { [
const CLOSE = new Set([0x7d, 0x5d]); // } ]
const QUOTE = 0x22;
const BACKSLASH = 0x5c;
const TICKS = "ticks";

/** The JSON value `stream` holds. Throws a SyntaxError if it is not JSON. */
export async function parseReplay(stream: ReadableStream<Uint8Array>): Promise<unknown> {
  const decoder = new TextDecoder();
  // Everything but the ticks, which are left as empty arrays, and each set's ticks.
  const rest: Uint8Array[] = [];
  const sets: unknown[][] = [];
  // The open containers' brackets; the ticks arrays are at [root, sets, set, ticks].
  const stack: number[] = [];
  let inString = false;
  let escaped = false;
  // The last string's bytes, kept only while it is short enough to be "ticks".
  let word: number[] | null = null;
  let inTicks = false;
  // The tick being read: its earlier chunks' bytes, and where it starts in this chunk.
  let tick: Uint8Array[] = [];
  let tickStart = -1;

  const reader = stream.getReader();
  for (;;) {
    const { done, value: chunk } = await reader.read();
    if (done) break;
    let restStart = inTicks ? -1 : 0;
    for (let i = 0; i < chunk.length; i++) {
      const b = chunk[i]!;
      if (inString) {
        if (escaped) escaped = false;
        else if (b === BACKSLASH) escaped = true;
        else if (b === QUOTE) inString = false;
        else if (word && word.length < TICKS.length) word.push(b);
        else word = null;
        continue;
      }
      if (b === QUOTE) {
        inString = true;
        word = [];
      } else if (OPEN.has(b)) {
        if (inTicks && stack.length === 4) tickStart = i;
        stack.push(b);
        if (stack.length === 4 && b === 0x5b && word && String.fromCharCode(...word) === TICKS) {
          // Keep the "[" in `rest`, then skip to the "]".
          rest.push(chunk.subarray(restStart, i + 1));
          restStart = -1;
          inTicks = true;
          sets.push([]);
        }
      } else if (CLOSE.has(b)) {
        stack.pop();
        if (inTicks && stack.length === 4) {
          tick.push(chunk.subarray(tickStart, i + 1));
          sets[sets.length - 1]!.push(JSON.parse(decoder.decode(concat(tick))));
          tick = [];
          tickStart = -1;
        } else if (inTicks && stack.length === 3) {
          inTicks = false;
          restStart = i;
        }
      }
    }
    if (tickStart >= 0) {
      tick.push(chunk.slice(tickStart));
      tickStart = 0;
    }
    if (restStart >= 0) rest.push(chunk.slice(restStart));
  }
  const replay = JSON.parse(decoder.decode(concat(rest)));
  if (Array.isArray(replay?.sets)) {
    replay.sets.forEach((s: { ticks?: unknown[] }, i: number) => {
      if (Array.isArray(s?.ticks) && sets[i]) s.ticks = sets[i];
    });
  }
  return replay;
}

function concat(parts: Uint8Array[]): Uint8Array {
  if (parts.length === 1) return parts[0]!;
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
