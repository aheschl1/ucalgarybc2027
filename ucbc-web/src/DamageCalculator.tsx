import { useState, type SubmitEvent } from "react";
import { ALGORITHMS, calculateDamage, mergedHealth, type Algorithm, type DamageInput } from "./damage";
import { dino, Sprite } from "./Tiles";
import { Button, Card, Field, Input, Page, Select, Table } from "./ui";

// each dino should have a level and health
type DinoFields = { level: string; health: string; attack: string; buff: string };
const fresh = (): DinoFields => ({ level: "1", health: "10", attack: "1", buff: "0" });
const format = (value: number) => value.toLocaleString("en-US", { maximumFractionDigits: 4 });

// combatant will be the attacker fields
// receiver is defendant fields
// this should allow people to have default levels to evaluate health
// and default attack stat
// can manually override for our purposes
// we don't have artifacts firmly in place so these will be inputted values
function Combatant({ receiver, value, onChange }: {
  receiver?: boolean;
  value: DinoFields;
  onChange: (value: DinoFields) => void;
}) {
  const name = receiver ? "Receiver" : "Attacker";
  return (
    <Card title={<><Sprite src={dino(receiver ? "b" : "a", Number(value.level) || 1)} className="!size-8" />{name}</>}>
      <div className="grid grid-cols-2 gap-4">
        <Field label={`${name} level`}>
          <Input type="number" required min={1} max={1000000} step={1} value={value.level}
            onChange={(event) => {
              const level = event.target.value;
              onChange({ ...value, level, health: level ? String(mergedHealth(Number(level))) : "", attack: level });
            }} />
        </Field>
        <Field label={`${name} health`}>
          <Input type="number" required min={0} max={50} step="any" value={value.health}
            onChange={(event) => onChange({ ...value, health: event.target.value })} />
        </Field>
        {!receiver && (
          <Field label="Attacker attack stat">
            <Input type="number" required min={0} max={1000000} step="any" value={value.attack}
              onChange={(event) => onChange({ ...value, attack: event.target.value })} />
          </Field>
        )}
        <Field label={receiver ? "Defense artifact buff" : "Attack artifact buff"}>
          <Input type="number" required min={0} max={1000000} step="any" value={value.buff}
            onChange={(event) => onChange({ ...value, buff: event.target.value })} />
        </Field>
      </div>
      <p className="mt-4 text-xs leading-relaxed text-muted">
        Changing level presets health to min(level × 10, 50){!receiver && " and attack stat to level"}.
        Override health for prior damage or splits.{!receiver && " Attacker health does not affect these formulas."}
      </p>
    </Card>
  );
}

export default function DamageCalculator() {
  const [attacker, setAttacker] = useState(fresh);
  const [receiver, setReceiver] = useState(fresh);
  const [algorithm, setAlgorithm] = useState<Algorithm>("old");
  const [result, setResult] = useState<{ input: DamageInput; algorithm: Algorithm } | null>(null);
  const selected = ALGORITHMS.find((item) => item.id === algorithm)!;
  const calculated = result && calculateDamage(result.algorithm, result.input);

  function calculate(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!event.currentTarget.reportValidity()) return;
    setResult({ algorithm, input: {
      attackerLevel: Number(attacker.level), attack: Number(attacker.attack), attackBuff: Number(attacker.buff),
      receiverLevel: Number(receiver.level), receiverHealth: Number(receiver.health), defenseBuff: Number(receiver.buff),
    } });
  }

  return (
    <Page wide title="Damage calculator" subtitle="Compare one-hit damage formulas before changing the game’s combat balance.">
      <p className="mb-6 max-w-3xl leading-relaxed text-muted">
        This is a sandbox, not reflective of changes to live rules. The engine currently has no separate attack stat or
        artifact damage/defense buffs, so those are your inputs. Health depends on a dino’s level and merging.
      </p>
      <form onSubmit={calculate} onChange={() => setResult(null)} className="space-y-6">
        <div className="grid gap-6 md:grid-cols-2">
          <Combatant value={attacker} onChange={setAttacker} />
          <Combatant receiver value={receiver} onChange={setReceiver} />
        </div>
        <Card title="Damage algorithm">
          <div className="max-w-md">
            <Field label="Algorithm">
              <Select value={algorithm} onChange={(event) => setAlgorithm(event.target.value as Algorithm)}>
                {ALGORITHMS.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
              </Select>
            </Field>
          </div>
          <p className="mt-4 break-words rounded-lg bg-panel p-3 font-mono text-xs leading-relaxed">{selected.formula}</p>
          <p className="mt-3 text-xs leading-relaxed text-muted">
            A = attacker attack stat; B = attack artifact buff; X = receiver level; F = defense artifact buff;
            D = |attacker level − receiver level − F|. Both multiplicative formulas use this same D;
            the second also subtracts F at the end.
          </p>
          <p className="mt-2 text-xs leading-relaxed text-muted">
            {algorithm === "old" ? "Old exactly follows rules.rs and ignores attack stat and both artifact buffs. " : ""}
            All algorithms floor damage to a whole number. Negative damage is forced to 0 as to not heal the enemy.
            Remaining health never drops below 0.
          </p>
          <Button type="submit" variant="accent" className="mt-5">Calculate</Button>
        </Card>
      </form>

      <div aria-live="polite" className="mt-6 space-y-6">
        {result && calculated && (
          <>
            <Card title={`Result · ${ALGORITHMS.find((item) => item.id === result.algorithm)!.name}`}>
              <div className="grid gap-5 sm:grid-cols-3">
                <div><p className="text-xs text-muted">Damage per hit</p><p className="mt-1 text-3xl font-semibold tabular-nums">{format(calculated.damage)}</p></div>
                <div><p className="text-xs text-muted">Actual health lost</p><p className="mt-1 text-3xl font-semibold tabular-nums">{format(calculated.healthLost)}</p></div>
                <div><p className="text-xs text-muted">Receiver health remaining</p><p className="mt-1 text-3xl font-semibold tabular-nums">{format(calculated.remainingHealth)}</p></div>
              </div>
              <p className="mt-4 text-muted">
                Receiver: {format(result.input.receiverHealth)} → {format(calculated.remainingHealth)} health.
                {calculated.remainingHealth === 0 ? " Receiver is defeated." : " Receiver survives."}
                {calculated.rawDamage < 0 && ` Raw formula result: ${format(calculated.rawDamage)}; clamped to 0 damage.`}
              </p>
            </Card>
            <Card title="Compare all algorithms · same inputs" flush>
              <Table head={["Algorithm", "Damage per hit", "Health lost", "Health remaining"]}>
                {ALGORITHMS.map((item) => {
                  const comparison = calculateDamage(item.id, result.input);
                  return (
                    <tr key={item.id} className={item.id === result.algorithm ? "bg-accent-soft" : ""}>
                      <th scope="row" className="px-4 py-3 font-medium">{item.name}</th>
                      <td className="px-4 py-3 tabular-nums">{format(comparison.damage)}</td>
                      <td className="px-4 py-3 tabular-nums">{format(comparison.healthLost)}</td>
                      <td className="px-4 py-3 tabular-nums">{format(comparison.remainingHealth)}</td>
                    </tr>
                  );
                })}
              </Table>
            </Card>
          </>
        )}
      </div>
    </Page>
  );
}
