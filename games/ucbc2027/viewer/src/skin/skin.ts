import { createContext, useContext } from "react";

import { assets, files, mapPictures } from "./assets.ts";

/** Every picture the sprites draw, shaped like `assets`. */
export type Skin = typeof assets;

/** A file in the assets folder, and the slots it fills: `dirt.png` fills several. */
export type Picture = { file: string; href: string; slots: string[] };

/** The sprites draw with `assets` unless a page provides another skin, as the skin editor does. */
export const SkinContext = createContext<Skin>(assets);

export function useSkin(): Skin {
  return useContext(SkinContext);
}

/** Each picture of `skin` with its slot, the path to it such as `dino.1.2.0`. */
export function slots(skin: Skin = assets): [slot: string, href: string][] {
  const found: [string, string][] = [];
  mapPictures(skin, (slot, href) => {
    found.push([slot, href]);
    return href;
  });
  return found;
}

/** `skin` with the pictures in `overrides` in their slots. */
export function withOverrides(overrides: Record<string, string>, skin: Skin = assets): Skin {
  return mapPictures(skin, (slot, href) => overrides[slot] ?? href) as Skin;
}

/** Every picture in `assets` by its file, with `file` relative to the assets folder. */
export function pictures(): Picture[] {
  const hrefs = new Map(slots());
  const found = new Map<string, Picture>();
  mapPictures(files, (slot, file) => {
    const picture = found.get(file) ?? { file, href: hrefs.get(slot)!, slots: [] };
    picture.slots.push(slot);
    found.set(file, picture);
    return file;
  });
  return [...found.values()];
}

/** `assets` with the pictures in `overrides`, keyed by file, in every slot they fill. */
export function withFiles(overrides: Record<string, string>): Skin {
  const bySlot = pictures().flatMap(({ file, slots }) => {
    const href = overrides[file];
    return href ? slots.map((slot): [string, string] => [slot, href]) : [];
  });
  return withOverrides(Object.fromEntries(bySlot));
}
