// @vitest-environment jsdom

import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { useFrame } from "./hooks.ts";

afterEach(cleanup);

it("throws outside a ViewerContext", () => {
  // React logs what a component throws.
  vi.spyOn(console, "error").mockImplementation(() => {});
  function Board() {
    useFrame();
    return null;
  }
  expect(() => render(<Board />)).toThrow("outside a ViewerContext");
  vi.restoreAllMocks();
});
