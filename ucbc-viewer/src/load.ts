// Long matches make replays larger than the longest string a browser allows, so a replay
// is parsed as it streams in: each set's ticks one at a time, and the rest once at the end.

const [QUOTE, BACKSLASH, COLON, COMMA] = [0x22, 0x5c, 0x3a, 0x2c];
const SPACE = new Set([0x20, 0x09, 0x0a, 0x0d]);
const [OBJECT, ARRAY, OBJECT_END, ARRAY_END] = [0x7b, 0x5b, 0x7d, 0x5d];
const LONGEST_KEY = 5;

/** The JSON value `stream` holds. Throws a SyntaxError if it is not JSON. */
export async function parseReplay(stream: ReadableStream<Uint8Array>): Promise<unknown> {
  const decoder = new TextDecoder();
  // Everything but the ticks, whose arrays are left empty, and each set's ticks.
  const rest: Uint8Array[] = [];
  const ticks: unknown[][] = [];
  // The open containers, and the key each object is at; ticks are at sets[i].ticks.
  const stack: number[] = [];
  const keys: (string | null)[] = [];
  let set = 0;
  let inString = false;
  let escaped = false;
  // The last string, while it is short and unescaped enough to be a key we look for.
  let word: number[] | null = null;
  let inTicks = false;
  // The ticks array's element being read: its earlier chunks, and where it starts in this one.
  let piece: Uint8Array[] = [];
  let pieceStart = -1;
  let pieces = 0;

  const endPiece = (chunk: Uint8Array, end: number, last: boolean) => {
    piece.push(chunk.subarray(pieceStart, end));
    const text = decoder.decode(concat(piece));
    if (!(last && pieces === 0 && !text.trim())) ticks[set]!.push(JSON.parse(text));
    pieces++;
    piece = [];
  };

  const reader = stream.getReader();
  try {
    for (;;) {
      const { done, value: chunk } = await reader.read();
      if (done) break;
      let restStart = inTicks ? -1 : 0;
      for (let i = 0; i < chunk.length; i++) {
        const b = chunk[i]!;
        if (inString) {
          if (escaped) escaped = false;
          else if (b === QUOTE) inString = false;
          else if (b === BACKSLASH) [escaped, word] = [true, null];
          else if (word && word.length < LONGEST_KEY) word.push(b);
          else word = null;
          continue;
        }
        if (b === QUOTE) {
          inString = true;
          word = [];
          continue;
        }
        if (SPACE.has(b)) continue;
        if (b === COLON && stack.at(-1) === OBJECT) {
          keys[stack.length - 1] = word && String.fromCharCode(...word);
        } else if (b === OBJECT || b === ARRAY) {
          stack.push(b);
          keys[stack.length - 1] = null;
          const inSets = stack[0] === OBJECT && stack[1] === ARRAY && keys[0] === "sets";
          if (stack.length === 2 && inSets) set = 0;
          if (stack.length === 4 && inSets && stack[2] === OBJECT && keys[2] === "ticks" && b === ARRAY) {
            // Keep the "[" in `rest`, then skip to the "]".
            rest.push(chunk.subarray(restStart, i + 1));
            restStart = -1;
            inTicks = true;
            ticks[set] = [];
            [pieceStart, pieces] = [i + 1, 0];
          }
        } else if (b === OBJECT_END || b === ARRAY_END) {
          stack.pop();
          if (inTicks && stack.length === 3) {
            endPiece(chunk, i, true);
            inTicks = false;
            restStart = i;
          }
        } else if (b === COMMA && inTicks && stack.length === 4) {
          endPiece(chunk, i, false);
          pieceStart = i + 1;
        } else if (b === COMMA && stack.length === 2 && keys[0] === "sets") {
          set++;
        }
        word = null;
      }
      if (inTicks) {
        piece.push(chunk.slice(pieceStart));
        pieceStart = 0;
      }
      if (restStart >= 0) rest.push(chunk.slice(restStart));
    }
  } catch (e) {
    reader.cancel().catch(() => {});
    throw e;
  }
  const replay = JSON.parse(decoder.decode(concat(rest)));
  if (Array.isArray(replay?.sets)) {
    replay.sets.forEach((s: { ticks?: unknown[] }, i: number) => {
      if (ticks[i] && Array.isArray(s?.ticks)) s.ticks = ticks[i];
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
