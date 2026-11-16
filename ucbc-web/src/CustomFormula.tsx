import { useEffect, useRef } from "react";
import { CUSTOM_VARIABLES } from "./damage";
import { Button, Field, Select } from "./ui";

// premade examples to select as defaults
const EXAMPLES = [
  ["Standard RPG", "A * 1.5 + B - abs(L - X - F)"],
  ["Subtractive defense", "max(1, A + B - D)"],
  ["Multiplicative defense", "(A + B) * X / (X + D)"],
  ["Multiplicative defense 2", "A * X / (X + D) + B - F"],
  ["Level gap squared", "max(1, (L - X)^2)"],
] as const;

// the editor and cursor and buttons
export default function CustomFormula({ value, onChange, error }: {
  value: string;
  onChange: (value: string) => void;
  error: string | null;
}) {
  const editor = useRef<HTMLTextAreaElement>(null);
  const cursor = useRef<number | null>(null);
  useEffect(() => {
    if (cursor.current === null) return;
    editor.current?.focus();
    editor.current?.setSelectionRange(cursor.current, cursor.current);
    cursor.current = null;
  }, [value]);

  function insert(token: string) {
    const start = editor.current?.selectionStart ?? value.length;
    const end = editor.current?.selectionEnd ?? value.length;
    cursor.current = start + token.length;
    onChange(value.slice(0, start) + token + value.slice(end));
  }

  return (
    <div className="mt-4 space-y-4">
      <div className="max-w-md">
        <Field label="Load example">
          <Select value="" onChange={(event) => onChange(event.target.value)}>
            <option value="" disabled>Choose a starting formula</option>
            {EXAMPLES.map(([name, formula]) => <option key={name} value={formula}>{name}</option>)}
          </Select>
        </Field>
      </div>
      <Field label="Custom formula">
        <textarea ref={editor} value={value} maxLength={500} rows={3} spellCheck={false}
          aria-invalid={!!error} aria-describedby="custom-formula-help custom-formula-error"
          onChange={(event) => onChange(event.target.value)}
          className="w-full rounded-lg border border-line bg-bg p-3 font-mono text-sm"
          placeholder="A * 1.5 + B - abs(L - X - F)" />
      </Field>
      <div className="flex flex-wrap gap-2" role="group" aria-label="Formula variables">
        {CUSTOM_VARIABLES.map(([symbol, description]) => (
          <Button key={symbol} type="button" variant="secondary" title={description}
            aria-label={`Insert ${symbol}: ${description}`} onClick={() => insert(symbol)}>{symbol}</Button>
        ))}
      </div>
      <div className="flex flex-wrap gap-2" role="group" aria-label="Formula operators and functions">
        {["+", "-", "*", "/", "%", "^", "(", ")", ",", "abs(", "floor(", "min(", "max("].map((token) => (
          <Button key={token} type="button" variant="secondary" onClick={() => insert(token)}>{token}</Button>
        ))}
        <Button type="button" variant="ghost" onClick={() => onChange("")}>Clear formula</Button>
      </div>
      <dl className="grid gap-x-6 gap-y-1 text-xs sm:grid-cols-2">
        {CUSTOM_VARIABLES.map(([symbol, description]) => (
          <div key={symbol} className="flex gap-2"><dt className="font-mono font-semibold">{symbol}</dt><dd className="text-muted">{description}</dd></div>
        ))}
      </dl>
      <p id="custom-formula-help" className="text-xs leading-relaxed text-muted">
        Type numbers and variables, or insert them at the cursor with the buttons. Use * for multiplication,<br />
        ^ for powers, and parentheses to group terms.<br />
        Functions: abs, floor, ceil, round, sqrt, pow, min, max.<br />
        Separate function arguments with commas.<br />
        Variable names are case-insensitive.<br />
        Maximum 500 characters.<br />
        Final damage is always floored and cannot be negative.
      </p>
      <div id="custom-formula-error">{error && <p role="alert" className="text-sm text-accent">{error}</p>}</div>
    </div>
  );
}
