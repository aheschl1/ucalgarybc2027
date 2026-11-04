import { createContext, useContext } from "react";

import { assets } from "./assets.ts";

/** Every picture the sprites draw, shaped like `assets`. */
export type Skin = typeof assets;

type Tree = string | readonly Tree[] | { readonly [key: string]: Tree };

/** The sprites draw with `assets` unless a page provides another skin, as the skin editor does. */
export const SkinContext = createContext<Skin>(assets);

export function useSkin(): Skin {
  return useContext(SkinContext);
}

function mapPictures(tree: Tree, f: (slot: string, href: string) => string, path = ""): Tree {
  if (typeof tree === "string") return f(path, tree);
  const at = (key: string | number) => (path ? `${path}.${key}` : String(key));
  if (Array.isArray(tree)) return tree.map((t, i) => mapPictures(t, f, at(i)));
  return Object.fromEntries(Object.entries(tree).map(([k, t]) => [k, mapPictures(t, f, at(k))]));
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
