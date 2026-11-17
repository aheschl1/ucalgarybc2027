import { expect, it } from "vitest";
import { evaluateExpression } from "./expression";

// AI Alert - the later of these were made using AI - they cover everything I'm concerned about
// But let me know of any bugs
it.each([
  ["2 + 3 * 4", 14], ["(2 + 3) * 4", 20], ["2^3^2", 512],
  ["-2^2", -4], ["(-2)^2", 4], ["2^-2", 0.25], ["--2", 2],
  ["7 % 4", 3], ["2 × 3 − 4 ÷ 2", 4], [".5 + 2. + 1e-2", 2.51],
  ["max(1, min(5, abs(-3)))", 3], ["floor(3.9) + ceil(1.1)", 5],
  ["round(2.6) + sqrt(9) + pow(2, 3)", 14], ["  a + b  ", 10],
])("evaluates math expression %s", (formula, expected) => {
  expect(evaluateExpression(formula, { A: 7, B: 3 })).toBeCloseTo(expected);
});

it.each([
  "", "1 / 0", "0 / 0", "4 % 0", "sqrt(-1)", "2^10000", "1e999", "pow(-1, .5)",
  "A +", "(A + 1", "1 2", "2(A)", "min()", "abs(1, 2)", "pow(2)",
  "min(1,)", "abs(2", "unknown + 1", "sin(1)",
  "window.alert(1)", "A.constructor", "constructor", "__proto__", "toString", "eval(1)",
  "A = 1", "1; 2", "[1]", "1 // 2", '"hello"',
])("rejects invalid or unsafe expression %s", (formula) => {
  expect(() => evaluateExpression(formula, { A: 7 })).toThrow();
});

it("bounds formula length and parser complexity", () => {
  expect(() => evaluateExpression("1".repeat(501), {})).toThrow("500 characters");
  expect(() => evaluateExpression("-".repeat(257) + "1", {})).toThrow("too complex");
});
