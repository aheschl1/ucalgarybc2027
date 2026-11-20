import { evaluateExpression } from "./expression";

/** Calculator-only proposals. The engine's combat rules are not changed here. */
export const ALGORITHMS = [
  { id: "old", name: "Old (2026-11-14)", formula: "Equal levels: 2; higher attacker: gap²; lower attacker: floor(gap² / 2). Minimum 1." },
  { id: "rpg", name: "Standard RPG", formula: "floor(A × 1.5 + B − D)" },
  { id: "subtractive", name: "Subtractive defense", formula: "max(1, floor(A + B − D))" },
  { id: "multiplicative", name: "Multiplicative defense", formula: "floor((A + B) × X / (X + D))" },
  { id: "multiplicative2", name: "Multiplicative defense 2", formula: "floor(A × X / (X + D) + B − F)" },
] as const;

export type Algorithm = (typeof ALGORITHMS)[number]["id"];
// variables for user input
export const CUSTOM_VARIABLES = [
  ["A", "Attacker attack stat"], ["B", "Attack artifact buff"],
  ["L", "Attacker level"], ["X", "Receiver level"],
  ["F", "Defense artifact buff"], ["D", "abs(L - X - F)"],
  ["AH", "Attacker health"], ["RH", "Receiver health"],
] as const;

export interface DamageInput {
  attackerLevel: number;
  attackerHealth: number;
  attack: number;
  attackBuff: number;
  receiverLevel: number;
  receiverHealth: number;
  defenseBuff: number;
}

/** Fresh dinos merged together, not a universal level-to-health mapping. */
export function mergedHealth(level: number): number {
  return Math.min(level * 10, 50);
}

export function calculateDamage(algorithm: Algorithm, input: DamageInput) {
  const { attackerLevel, attack, attackBuff, receiverLevel, defenseBuff } = input;
  const defense = Math.abs(attackerLevel - receiverLevel - defenseBuff);
  let rawDamage: number;
  switch (algorithm) {
    case "old": {
      // games/ucbc2027/src/rules.rs::damage, as of 2026-11-14.
      const gapSquared = (attackerLevel - receiverLevel) ** 2;
      rawDamage = Math.max(1, attackerLevel === receiverLevel ? 2
        : attackerLevel > receiverLevel ? gapSquared : Math.floor(gapSquared / 2));
      break;
    }
    case "rpg":
      rawDamage = Math.floor(attack * 1.5 + attackBuff - defense);
      break;
    case "subtractive":
      rawDamage = Math.max(1, Math.floor(attack + attackBuff - defense));
      break;
    case "multiplicative":
      rawDamage = Math.floor((attack + attackBuff) * receiverLevel / (receiverLevel + defense));
      break;
    case "multiplicative2":
      rawDamage = Math.floor(attack * receiverLevel / (receiverLevel + defense) + attackBuff - defenseBuff);
      break;
  }
  return damageResult(rawDamage, input);
}

// custom algorithm
export function calculateCustomDamage(formula: string, input: DamageInput) {
  const rawDamage = evaluateExpression(formula, {
    A: input.attack, B: input.attackBuff, L: input.attackerLevel,
    X: input.receiverLevel, F: input.defenseBuff,
    D: Math.abs(input.attackerLevel - input.receiverLevel - input.defenseBuff),
    AH: input.attackerHealth, RH: input.receiverHealth,
  });
  return damageResult(Math.floor(rawDamage), input);
}
// the resulting damage
function damageResult(rawDamage: number, input: DamageInput) {
  const { attackerLevel, receiverLevel, receiverHealth, defenseBuff } = input;
  const defense = Math.abs(attackerLevel - receiverLevel - defenseBuff);
  // Every formula floors final damage. A negative proposal is a blocked hit,
  // not healing I forgot negative damage does that.
  const damage = Math.max(0, rawDamage);
  const remainingHealth = Math.max(0, receiverHealth - damage);
  return { rawDamage, damage, remainingHealth, healthLost: receiverHealth - remainingHealth, defense };
}
