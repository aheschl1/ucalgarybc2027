// Every picture on the board, from games/ucbc2027/viewer/assets. To change one, replace
// its file or point its line at a new one. Lists are per team, indexed by team. A picture
// is drawn into its box (one tile, or a lab's 2x2 tiles) keeping its aspect ratio.

import dino0 from "../../assets/dino-0.svg";
import dino1 from "../../assets/dino-1.svg";
import empty from "../../assets/empty.svg";
import fossil from "../../assets/fossil.svg";
import lab0 from "../../assets/lab-0.svg";
import lab1 from "../../assets/lab-1.svg";
import labTile0 from "../../assets/lab-tile-0.svg";
import labTile1 from "../../assets/lab-tile-1.svg";
import wall from "../../assets/wall.svg";

export const assets = {
  /** Tiles. */
  empty,
  wall,
  labTile: [labTile0, labTile1],
  /** A fossil, lying on a tile or held by a dino. */
  fossil,
  /** Units. */
  lab: [lab0, lab1],
  dino: [dino0, dino1],
};
