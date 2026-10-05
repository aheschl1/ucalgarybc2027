// Every picture on the board, from games/ucbc2027/viewer/assets. To change one, replace its file
// or point its line at a new one. Lists are per team, indexed by team. A picture is drawn
// into its box (one tile, or a lab's 2x2 tiles) keeping its aspect ratio.
//
// The art is 16x16 RGBA pixel art, one tile per picture, drawn unsmoothed (skin.css).
// Labs and the artifact also come at 64x64, for boxes bigger than a tile.
//
// Dinos are `dino_<team>/<team><tier><frame>.png`: team `a` or `b`, tier 0-3 (more
// detail with each tier), frame 1-2 of an idle loop. All face right, feet on the bottom row.

import a01 from "../../assets/dino/dino_a/a01.png";
import a02 from "../../assets/dino/dino_a/a02.png";
import a11 from "../../assets/dino/dino_a/a11.png";
import a12 from "../../assets/dino/dino_a/a12.png";
import a21 from "../../assets/dino/dino_a/a21.png";
import a22 from "../../assets/dino/dino_a/a22.png";
import a31 from "../../assets/dino/dino_a/a31.png";
import a32 from "../../assets/dino/dino_a/a32.png";
import b01 from "../../assets/dino/dino_b/b01.png";
import b02 from "../../assets/dino/dino_b/b02.png";
import b11 from "../../assets/dino/dino_b/b11.png";
import b12 from "../../assets/dino/dino_b/b12.png";
import b21 from "../../assets/dino/dino_b/b21.png";
import b22 from "../../assets/dino/dino_b/b22.png";
import b31 from "../../assets/dino/dino_b/b31.png";
import b32 from "../../assets/dino/dino_b/b32.png";
import dirt from "../../assets/map/environment/dirt.png";
import lava from "../../assets/map/environment/lava.png";
import wall from "../../assets/map/environment/wall.png";
import artifact from "../../assets/map/items/artifacts/artifact64.png";
import beacon from "../../assets/map/items/beacon.png";
import fossil from "../../assets/map/items/fossil.png";
import aLab from "../../assets/map/labs/aLab64.png";
import bLab from "../../assets/map/labs/bLab64.png";

export const assets = {
  /** Tiles. */
  empty: dirt,
  wall,
  /** Under a lab, hidden while the lab stands. */
  labTile: [dirt, dirt],
  /** A fossil, lying on a tile or held by a dino. */
  fossil,
  /** Units. */
  lab: [aLab, bLab],
  /** `dino[team][tier][frame]`, tier from the dino's level (`dinoTier`). */
  dino: [
    [
      [a01, a02],
      [a11, a12],
      [a21, a22],
      [a31, a32],
    ],
    [
      [b01, b02],
      [b11, b12],
      [b21, b22],
      [b31, b32],
    ],
  ],
  /** Drawn but not in the game yet. */
  unused: { lava, beacon, artifact },
};

/** The picture tier for a dino level: level 1 is tier 0, and the top tier holds every
 * level past it. */
export function dinoTier(level: number): number {
  return Math.min(Math.max(level - 1, 0), assets.dino[0]!.length - 1);
}
