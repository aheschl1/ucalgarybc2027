// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import DamageCalculator from "./DamageCalculator";

afterEach(cleanup);
// i think this is how you do it?
it("evaluates custom formulas using current stats and compares against presets", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "custom" } });
  fireEvent.change(screen.getByLabelText("Attacker health"), { target: { value: "20" } });
  fireEvent.change(screen.getByLabelText("Custom formula"), { target: { value: "AH / 3 + B" } });
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByRole("heading", { name: "Result · Custom" })).toBeTruthy();
  expect(screen.getByText("Receiver: 10 -> 4 health. Receiver survives.")).toBeTruthy();
  expect(screen.getAllByRole("row")).toHaveLength(7);
});

it("shows custom formula errors and areas", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "custom" } });
  fireEvent.change(screen.getByLabelText("Custom formula"), { target: { value: "A / 0" } });
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByRole("alert").textContent).toContain("finite number");
  expect(screen.queryByText("Actual health lost")).toBeNull();
  fireEvent.change(screen.getByLabelText("Custom formula"), { target: { value: "2.9" } });
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByText("Receiver: 10 -> 8 health. Receiver survives.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Clear formula" }));
  expect(screen.queryByText("Actual health lost")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByRole("alert").textContent).toBe("Enter a formula.");
});

it("inserts variables at the cursor and loads editable examples", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "custom" } });
  const editor = screen.getByLabelText("Custom formula") as HTMLTextAreaElement;
  fireEvent.change(editor, { target: { value: "2 * " } });
  editor.setSelectionRange(4, 4);
  fireEvent.click(screen.getByRole("button", { name: "Insert A: Attacker attack stat" }));
  expect(editor.value).toBe("2 * A");
  expect(editor.selectionStart).toBe(5);
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByText("Receiver: 10 -> 8 health. Receiver survives.")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Load example"), { target: { value: "max(1, A + B - D)" } });
  expect(editor.value).toBe("max(1, A + B - D)");
  expect(screen.queryByText("Actual health lost")).toBeNull();
});

it("calculates on submission and compares all five algorithms", () => {
  render(<DamageCalculator />);
  expect(screen.queryByText("Damage per hit")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  const result = screen.getByRole("heading", { name: "Result · Old (2026-11-14)" }).closest("section")!;
  expect(within(result).getAllByText("2")).toHaveLength(2);
  expect(within(result).getByText("Receiver: 10 -> 8 health. Receiver survives.")).toBeTruthy();
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
  expect(screen.getByText("Receiver: 3 -> 1 health. Receiver survives.")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Defense artifact buff"), { target: { value: "1" } });
  expect(screen.queryByText("Actual health lost")).toBeNull();
});

it("uses artifact inputs in proposals and forces lethal hits to zero (non negative)", () => {
  render(<DamageCalculator />);
  fireEvent.change(screen.getByLabelText("Attack artifact buff"), { target: { value: "10" } });
  fireEvent.change(screen.getByLabelText("Defense artifact buff"), { target: { value: "2" } });
  fireEvent.change(screen.getByLabelText("Algorithm"), { target: { value: "rpg" } });
  fireEvent.change(screen.getByLabelText("Receiver health"), { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(screen.getByText("Receiver: 5 -> 0 health. Receiver is defeated.")).toBeTruthy();
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
