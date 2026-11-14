// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import DamageCalculator from "./DamageCalculator";

afterEach(cleanup);

it("calculates on submission and compares all five algorithms", () => {
  render(<DamageCalculator />);
  expect(screen.queryByText("Damage per hit")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  const result = screen.getByRole("heading", { name: "Result · Old (2026-11-14)" }).closest("section")!;
  expect(within(result).getAllByText("2")).toHaveLength(2);
  expect(within(result).getByText("Receiver: 10 → 8 health. Receiver survives.")).toBeTruthy();
  expect(screen.getAllByRole("row")).toHaveLength(6);
});

it("presets stats on level changes, allows overrides, and clears stale results", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Attacker level"), { target: { value: "6" } });
  expect((screen.getByLabelText("Attacker health") as HTMLInputElement).value).toBe("50");
  expect((screen.getByLabelText("Attacker attack stat") as HTMLInputElement).value).toBe("6");
  fireEvent.change(screen.getByLabelText("Attacker attack stat"), { target: { value: "7" } });
  fireEvent.change(screen.getByLabelText("Receiver health"), { target: { value: "3" } });
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "subtractive" } });
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByText("Receiver: 3 → 1 health. Receiver survives.")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Defense artifact buff"), { target: { value: "1" } });
  expect(screen.queryByText("Actual health lost")).toBeNull();
});

it("uses artifact inputs in proposals and clamps lethal hits to zero health", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Attack artifact buff"), { target: { value: "10" } });
  fireEvent.change(screen.getByLabelText("Defense artifact buff"), { target: { value: "2" } });
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "rpg" } });
  fireEvent.change(screen.getByLabelText("Receiver health"), { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByText("Receiver: 5 → 0 health. Receiver is defeated.")).toBeTruthy();
});

it("rejects empty, zero-level, negative, and over-cap inputs", () => {
  render(<DamageCalculator />);
  for (const [label, value] of [["Attacker level", ""], ["Receiver level", "0"],
    ["Attack artifact buff", "-1"], ["Receiver health", "51"]]) {
    const field = screen.getByLabelText(label!) as HTMLInputElement;
    const previous = field.value;
    fireEvent.change(field, { target: { value } });
    expect(field.checkValidity()).toBe(false);
    fireEvent.submit(screen.getByRole("button", { name: "Calculate" }).closest("form")!);
    expect(screen.queryByText("Actual health lost")).toBeNull();
    fireEvent.change(field, { target: { value: previous } });
  }
});
