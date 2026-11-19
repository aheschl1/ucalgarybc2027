/** A math-only parser - KEEP IN MIND SECURITY - formulas are tokenized and evaluated without JavaScript execution */
export function evaluateExpression(source: string, variables: Record<string, number>): number {
  if (source.length > 500) throw new Error("Use at most 500 characters.");
  // normalize them for parsing.
  const text = source.replaceAll("×", "*").replaceAll("÷", "/").replaceAll("−", "-").trim();
  if (!text) throw new Error("Enter a formula.");
  // match only numbers, variable names, operators, parentheses, and commas
  const pattern = /\s*((?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?|[a-z_][a-z_0-9]*|[+\-*/%^(),])/iy;
  const tokens: string[] = [];
  let offset = 0;
  while (offset < text.length) {
    pattern.lastIndex = offset;
    const match = pattern.exec(text);
    // tell error at source position of the first unsupported character.
    if (!match) throw new Error(`Unexpected character at position ${offset + 1}.`);
    tokens.push(match[1]!);
    offset = pattern.lastIndex;
    if (tokens.length > 256) throw new Error("Formula is too complex (maximum 256 tokens).");
  }

  let position = 0;
  const peek = () => tokens[position];
  const take = (token: string) => {
    if (peek() !== token) return false;
    position++;
    return true;
  };

  // stop formulas that overflow or produce NaN/Infinity
  const finite = (value: number): number => {
    if (!Number.isFinite(value)) throw new Error("Formula must produce a finite number; check division by zero, roots, or powers.");
    return value;
  };

  // parse addition and subtraction from left to right.
  function sum(): number {
    let value = product();
    while (peek() === "+" || peek() === "-") {
      const operator = tokens[position++];
      const right = product();
      value = finite(operator === "+" ? value + right : value - right);
    }
    return value;
  }

  // parse multiplication, division, and modulo before sums.
  function product(): number {
    let value = unary();
    while (peek() === "*" || peek() === "/" || peek() === "%") {
      const operator = tokens[position++];
      const right = unary();
      value = finite(operator === "*" ? value * right : operator === "/" ? value / right : value % right);
    }
    return value;
  }

  function unary(): number {
    if (take("+")) return unary();
    if (take("-")) return -unary();
    const value = primary();
    // Exponentiation is right-associative
    return take("^") ? finite(value ** unary()) : value;
  }
  // parse groups, literals, variables, and whitelisted functions.
  function primary(): number {
    if (take("(")) {
      const value = sum();
      if (!take(")")) throw new Error("Missing closing parenthesis.");
      return value;
    }
    // tokenization parsing
    const token = tokens[position++];
    if (!token) throw new Error("Expected a number, variable, or function.");
    if (/^(?:\d|\.)/.test(token)) return finite(Number(token));
    if (!/^[a-z_]/i.test(token)) throw new Error(`Unexpected '${token}'.`);
    if (!take("(")) {
      const name = token.toUpperCase();
      if (!Object.hasOwn(variables, name)) throw new Error(`Unknown variable '${token}'. Use the variable buttons below.`);
      return finite(variables[name]!);
    }
    const args: number[] = [];
    if (!take(")")) {
      do { args.push(sum()); } while (take(","));
      if (!take(")")) throw new Error("Missing closing parenthesis or comma in function.");
    }
    // function names are parsed
    const name = token.toLowerCase();
    switch (name) {
      case "min":
      case "max":
        if (args.length < 1) throw new Error(`${name} needs at least one argument.`);
        return name === "min" ? Math.min(...args) : Math.max(...args);
      case "pow":
        if (args.length !== 2) throw new Error("power needs two arguments.");
        return finite(args[0]! ** args[1]!);
      case "abs":
      case "floor":
      case "ceil":
      case "round":
      case "sqrt":
        if (args.length !== 1) throw new Error(`${name} needs one argument.`);
        return finite(Math[name](args[0]!));
      default:
        throw new Error(`Unknown function '${token}'.`);
    }
  }

  const value = finite(sum());
  if (position !== tokens.length) throw new Error(`Unexpected '${peek()}'. Use * for multiplication.`);
  return value;
}
