import { describe, expect, it } from "vitest";
import { ALGORITHMS, calculateDamage, mergedHealth, type DamageInput } from "./damage";

const input: DamageInput = {
  attackerLevel: 5, attack: 7, attackBuff: 3,
  receiverLevel: 3, receiverHealth: 20, defenseBuff: 4,
};

describe("Old (2026-11-14)", () => {
  it.each([
    [1, 1, 2], [3, 3, 2], [2, 1, 1], [1, 2, 1],
    [3, 1, 4], [4, 1, 9], [1, 3, 2], [1, 4, 4],
  ])("matches rules.rs for level %i attacking %i", (attackerLevel, receiverLevel, expected) => {
    expect(calculateDamage("old", { ...input, attackerLevel, receiverLevel }).damage).toBe(expected);
  });

  it("ignores experimental attack stats and artifacts", () => {
    expect(calculateDamage("old", input).damage).toBe(4);
    expect(calculateDamage("old", { ...input, attack: 0, attackBuff: 999, defenseBuff: 999 }).damage).toBe(4);
  });
});

describe("proposed formulas", () => {
  it("takes the absolute value of the entire defense expression", () => {
    expect(calculateDamage("rpg", input).defense).toBe(2);
    expect(calculateDamage("rpg", { ...input, attackerLevel: 1 }).defense).toBe(6);
  });

  it("floors Standard RPG after the multiplier, buff, and defense", () => {
    expect(calculateDamage("rpg", input).damage).toBe(11);
  });

  it("applies subtractive defense and its minimum of 1", () => {
    expect(calculateDamage("subtractive", input).damage).toBe(8);
    expect(calculateDamage("subtractive", { ...input, attack: 7.9 }).damage).toBe(8);
    expect(calculateDamage("subtractive", { ...input, attack: 0, attackBuff: 0 }).damage).toBe(1);
  });

  it("multiplies the attack buff in Multiplicative defense", () => {
    expect(calculateDamage("multiplicative", input).damage).toBe(6);
    expect(calculateDamage("multiplicative", { ...input, attack: 1, attackBuff: 0 }).damage).toBe(0);
  });

  it("applies buffs outside the fraction in Multiplicative defense 2", () => {
    expect(calculateDamage("multiplicative2", input).damage).toBe(3);
  });

  it.each(["rpg", "multiplicative2"] as const)("clamps negative %s damage instead of healing", (algorithm) => {
    const result = calculateDamage(algorithm, { ...input, attack: 0, attackBuff: 0 });
    expect(result.rawDamage).toBeLessThan(0);
    expect(result.damage).toBe(0);
    expect(result.remainingHealth).toBe(20);
  });

  it.each(ALGORITHMS)("saturates health at zero for $name", ({ id }) => {
    const result = calculateDamage(id, { ...input, receiverHealth: 1 });
    expect(result.remainingHealth).toBe(0);
    expect(result.healthLost).toBe(1);
  });

  it("does not round fractional health before calculating", () => {
    const result = calculateDamage("multiplicative", { ...input, receiverHealth: 19.4 });
    expect(result.remainingHealth).toBeCloseTo(13.4);
    expect(result.healthLost).toBe(6);
  });

  it("uses full attack when defense is zero", () => {
    expect(calculateDamage("multiplicative", { ...input, attackerLevel: 3, defenseBuff: 0 }).damage).toBe(10);
  });

  it("presets merged health with the engine's 50 HP cap", () => {
    expect([1, 2, 5, 6, 100].map(mergedHealth)).toEqual([10, 20, 50, 50, 50]);
  });
});
