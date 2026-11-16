// Every picture on the board, by its PNG under games/ucbc2027/viewer/assets. To change one,
// replace its file or point its line at a new one. Lists are per team, indexed by team. A picture is drawn
// into its box (one tile, or a lab's 2x2 tiles) keeping its aspect ratio.
//
// The art is 16x16 RGBA pixel art, one tile per picture, drawn unsmoothed (skin.css).
// Labs and the artifact also come at 64x64, for boxes bigger than a tile.
//
// Dinos are `dino_<team>/<team><tier><frame>.png`: team `a` or `b`, tier 0-3 (more
// detail with each tier), frame 1-2 of an idle loop. All face right, feet on the bottom row.

type Tree = string | readonly Tree[] | { readonly [key: string]: Tree };

const FOLDER = "../../assets/";
const urls = import.meta.glob<string>("../../assets/**/*.png", { eager: true, query: "?url", import: "default" });

/** `tree` with each picture passed through `f`, along with its slot: its path, such as `dino.1.2.0`. */
export function mapPictures(tree: Tree, f: (slot: string, picture: string) => string, path = ""): Tree {
  if (typeof tree === "string") return f(path, tree);
  const at = (key: string | number) => (path ? `${path}.${key}` : String(key));
  if (Array.isArray(tree)) return tree.map((t, i) => mapPictures(t, f, at(i)));
  return Object.fromEntries(Object.entries(tree).map(([k, t]) => [k, mapPictures(t, f, at(k))]));
}

const dinos = (team: string) =>
  [0, 1, 2, 3].map((tier) => [1, 2].map((frame) => `dino/dino_${team}/${team}${tier}${frame}.png`));

export const files = {
  /** Tiles. */
  empty: "map/environment/dirt.png",
  wall: "map/environment/wall.png",
  /** Under a lab, hidden while the lab stands. */
  labTile: ["map/environment/dirt.png", "map/environment/dirt.png"],
  /** A fossil, lying on a tile or held by a dino. */
  fossil: "map/items/fossil.png",
  /** Units. */
  lab: ["map/labs/aLab64.png", "map/labs/bLab64.png"],
  /** `dino[team][tier][frame]`, tier from the dino's level (`dinoTier`). */
  dino: [dinos("a"), dinos("b")],
  /** Drawn but not in the game yet. */
  unused: {
    lava: "map/environment/lava.png",
    beacon: "map/items/beacon.png",
    artifact: "map/items/artifacts/artifact64.png",
  },
};

/** The URL of each picture in `files`. */
export const assets = mapPictures(files, (slot, file) => {
  const url = urls[FOLDER + file];
  if (!url) throw new Error(`${slot}: no ${file} in the assets folder`);
  return url;
}) as typeof files;

/** The picture tier for a dino level: level 1 is tier 0, and the top tier holds every
 * level past it. */
export function dinoTier(level: number): number {
  return Math.min(Math.max(level - 1, 0), assets.dino[0]!.length - 1);
}
