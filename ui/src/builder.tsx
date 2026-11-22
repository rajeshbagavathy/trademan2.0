import type { ReactNode } from "react";
import { legDraft, UNDERLYINGS, type Definition, type LegDraft, type Overall, type SlKind } from "./types";

const control = "field";

export function Builder({
  draft,
  onChange,
  onSave,
  onCancel,
}: {
  draft: Definition;
  onChange: (next: Definition) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  function patch(partial: Partial<Definition>) {
    onChange({ ...draft, ...partial });
  }
  function patchLeg(index: number, partial: Partial<LegDraft>) {
    const legs = draft.legs.map((leg, i) => (i === index ? { ...leg, ...partial } : leg));
    patch({ legs });
  }
  function patchOverall(partial: Partial<Overall>) {
    patch({ overall: { ...draft.overall, ...partial } });
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-auto px-5 py-4">
        <div className="mb-4 flex items-end justify-between gap-4">
          <div>
            <h1 className="text-lg font-semibold tracking-tight">New strategy</h1>
            <p className="text-sm text-slate-500">Add each leg, then set the strategy-level target, stop, and trail.</p>
          </div>
          <button className="text-sm text-slate-500 hover:text-slate-800" onClick={onCancel} type="button">
            Back to strategies
          </button>
        </div>

        <div className="grid gap-3 rounded-xl border border-slate-200 bg-white p-4 md:grid-cols-4">
          <Label text="Name">
            <input className={control} value={draft.name} onChange={(event) => patch({ name: event.target.value })} />
          </Label>
          <Label text="Index">
            <select className={control} value={draft.underlying} onChange={(event) => patch({ underlying: event.target.value })}>
              {UNDERLYINGS.map((name) => (
                <option key={name}>{name}</option>
              ))}
            </select>
          </Label>
          <Label text="Underlying from">
            <select className={control} value={draft.underlying_from} onChange={(event) => patch({ underlying_from: event.target.value as Definition["underlying_from"] })}>
              <option value="cash">Cash</option>
              <option value="futures">Futures</option>
            </select>
          </Label>
          <Label text="Strategy type">
            <select className={control} value={draft.strategy_type} onChange={(event) => patch({ strategy_type: event.target.value as Definition["strategy_type"] })}>
              <option value="intraday">Intraday</option>
              <option value="btst">BTST</option>
              <option value="positional">Positional</option>
            </select>
          </Label>
          <Label text="Entry time">
            <input className={control} type="time" value={draft.entry_time} onChange={(event) => patch({ entry_time: event.target.value })} />
          </Label>
          <Label text="Exit time">
            <input className={control} type="time" value={draft.exit_time} onChange={(event) => patch({ exit_time: event.target.value })} />
          </Label>
          <Label text="Square off">
            <select className={control} value={draft.square_off} onChange={(event) => patch({ square_off: event.target.value as Definition["square_off"] })}>
              <option value="partial">Partial</option>
              <option value="complete">Complete</option>
            </select>
          </Label>
          <Label text="Trail SL to breakeven">
            <select className={control} value={draft.breakeven} onChange={(event) => patch({ breakeven: event.target.value as Definition["breakeven"] })}>
              <option value="none">Off</option>
              <option value="all">All legs</option>
              <option value="sl">SL legs</option>
            </select>
          </Label>
        </div>

        <div className="mt-4 space-y-3">
          {draft.legs.map((leg, index) => (
            <section key={index} className="rounded-xl border border-slate-200 bg-white p-4">
              <div className="mb-3 flex items-center justify-between">
                <h2 className="text-sm font-semibold">Leg {index + 1}</h2>
                {draft.legs.length > 1 && (
                  <button className="text-xs text-slate-500 hover:text-rose-600" type="button" onClick={() => patch({ legs: draft.legs.filter((_, i) => i !== index) })}>
                    Remove
                  </button>
                )}
              </div>
              <div className="grid gap-3 md:grid-cols-6">
                <Label text="Position">
                  <select className={control} value={leg.position} onChange={(event) => patchLeg(index, { position: event.target.value as LegDraft["position"] })}>
                    <option value="sell">Sell</option>
                    <option value="buy">Buy</option>
                  </select>
                </Label>
                <Label text="Option type">
                  <select className={control} value={leg.option_type} onChange={(event) => patchLeg(index, { option_type: event.target.value as LegDraft["option_type"] })}>
                    <option value="CE">CE</option>
                    <option value="PE">PE</option>
                  </select>
                </Label>
                <Label text="Expiry">
                  <select className={control} value={leg.expiry} onChange={(event) => patchLeg(index, { expiry: event.target.value as LegDraft["expiry"] })}>
                    <option value="weekly">Weekly</option>
                    <option value="next">Next weekly</option>
                    <option value="monthly">Monthly</option>
                    <option value="next_monthly">Next monthly</option>
                  </select>
                </Label>
                <Label text="Strike criteria">
                  <select className={control} value="strike" onChange={() => undefined}>
                    <option value="strike">Strike type</option>
                  </select>
                </Label>
                <Label text="Strike type">
                  <select className={control} value={leg.strike} onChange={(event) => patchLeg(index, { strike: event.target.value as LegDraft["strike"] })}>
                    <option value="itm">ITM</option>
                    <option value="atm">ATM</option>
                    <option value="otm">OTM</option>
                  </select>
                </Label>
                <Label text="Lots">
                  <input className={control} type="number" min={1} value={leg.lots} onChange={(event) => patchLeg(index, { lots: Number(event.target.value) })} />
                </Label>
              </div>
              <div className="mt-3 grid gap-3 border-t border-slate-100 pt-3 md:grid-cols-6">
                <Label text="Stop loss">
                  <select className={control} value={leg.sl} onChange={(event) => patchLeg(index, { sl: event.target.value as SlKind })}>
                    <option value="none">None</option>
                    <option value="points">Points</option>
                    <option value="underlying_points">Underlying points</option>
                    <option value="percent">Percent</option>
                    <option value="underlying_percent">Underlying %</option>
                    <option value="delta">Delta (pts)</option>
                  </select>
                </Label>
                <Label text="SL value">
                  <input className={control} type="number" disabled={leg.sl === "none"} value={leg.sl_value} onChange={(event) => patchLeg(index, { sl_value: Number(event.target.value) })} />
                </Label>
                <Label text="Trail SL">
                  <select className={control} value={leg.trail} onChange={(event) => patchLeg(index, { trail: event.target.value as LegDraft["trail"] })}>
                    <option value="none">None</option>
                    <option value="points">Points</option>
                  </select>
                </Label>
                {leg.trail === "points" && (
                  <>
                    <Label text="For every">
                      <input className={control} type="number" value={leg.trail_every} onChange={(event) => patchLeg(index, { trail_every: Number(event.target.value) })} />
                    </Label>
                    <Label text="Trail by">
                      <input className={control} type="number" value={leg.trail_by} onChange={(event) => patchLeg(index, { trail_by: Number(event.target.value) })} />
                    </Label>
                  </>
                )}
                <Label text="Target profit">
                  <select className={control} value={leg.target} onChange={(event) => patchLeg(index, { target: event.target.value as SlKind })}>
                    <option value="none">None</option>
                    <option value="points">Points</option>
                    <option value="underlying_points">Underlying points</option>
                    <option value="percent">Percent</option>
                    <option value="underlying_percent">Underlying %</option>
                    <option value="delta">Delta (pts)</option>
                  </select>
                </Label>
              </div>
              {leg.sl.startsWith("underlying") || leg.sl === "delta" ? (
                <p className="mt-2 text-xs text-slate-500">Points and percent move the broker stop from the fill. Underlying and delta are saved on the leg.</p>
              ) : null}
              <div className="mt-3 grid gap-3 md:grid-cols-4">
                <Label text="Simple momentum">
                  <select className={control} value={leg.momentum} onChange={(event) => patchLeg(index, { momentum: event.target.value as LegDraft["momentum"] })}>
                    <option value="none">None</option>
                    <option value="percent_up">Premium % up</option>
                    <option value="percent_down">Premium % down</option>
                  </select>
                </Label>
                {leg.momentum !== "none" && (
                  <Label text="Premium %">
                    <input className={control} type="number" min={0} value={leg.momentum_value} onChange={(event) => patchLeg(index, { momentum_value: Number(event.target.value) })} />
                  </Label>
                )}
                <Label text="Range breakout">
                  <select className={control} value={leg.range_breakout} onChange={(event) => patchLeg(index, { range_breakout: event.target.value as LegDraft["range_breakout"] })}>
                    <option value="none">None</option>
                    <option value="high">High</option>
                    <option value="low">Low</option>
                    <option value="underlying">Underlying</option>
                    <option value="strike">Strike price</option>
                  </select>
                </Label>
              </div>
            </section>
          ))}
          <button
            className="h-10 w-full rounded-xl border border-dashed border-slate-300 text-sm font-medium text-slate-600 hover:bg-white"
            type="button"
            onClick={() => patch({ legs: [...draft.legs, legDraft(draft.legs.length % 2 === 0 ? "CE" : "PE")] })}
          >
            Add leg
          </button>
        </div>
      </div>

      <section className="shrink-0 border-t border-slate-200 bg-white px-5 py-4">
        <h2 className="text-sm font-semibold">Overall strategy settings</h2>
        <div className="mt-3 grid gap-3 md:grid-cols-4">
          <Label text="Overall target">
            <select className={control} value={draft.overall.target} onChange={(event) => patchOverall({ target: event.target.value as Overall["target"] })}>
              <option value="none">None</option>
              <option value="mtm">MTM</option>
              <option value="premium">Total premium %</option>
            </select>
          </Label>
          <Label text="Target value">
            <input className={control} type="number" disabled={draft.overall.target === "none"} value={draft.overall.target_value} onChange={(event) => patchOverall({ target_value: Number(event.target.value) })} />
          </Label>
          <Label text="Overall stop loss">
            <select className={control} value={draft.overall.stop} onChange={(event) => patchOverall({ stop: event.target.value as Overall["stop"] })}>
              <option value="none">None</option>
              <option value="mtm">MTM</option>
              <option value="premium">Total premium %</option>
            </select>
          </Label>
          <Label text="Stop value">
            <input className={control} type="number" disabled={draft.overall.stop === "none"} value={draft.overall.stop_value} onChange={(event) => patchOverall({ stop_value: Number(event.target.value) })} />
          </Label>
          <Label text="Trailing options">
            <select className={control} value={draft.overall.trailing} onChange={(event) => patchOverall({ trailing: event.target.value as Overall["trailing"] })}>
              <option value="none">None</option>
              <option value="lock">Lock</option>
              <option value="lock_trail">Lock and trail</option>
              <option value="overall_trail">Overall trail SL</option>
            </select>
          </Label>
          {(draft.overall.trailing === "lock" || draft.overall.trailing === "lock_trail") && (
            <>
              <Label text="If profit reaches">
                <input className={control} type="number" value={draft.overall.lock_at} onChange={(event) => patchOverall({ lock_at: Number(event.target.value) })} />
              </Label>
              <Label text="Lock profit">
                <input className={control} type="number" value={draft.overall.lock_profit} onChange={(event) => patchOverall({ lock_profit: Number(event.target.value) })} />
              </Label>
            </>
          )}
          <Label text="Overall momentum">
            <select className={control} value={draft.overall.momentum} onChange={(event) => patchOverall({ momentum: event.target.value as Overall["momentum"] })}>
              <option value="none">None</option>
              <option value="points_up">Points up</option>
              <option value="points_down">Points down</option>
              <option value="percent_up">Percent up</option>
              <option value="percent_down">Percent down</option>
            </select>
          </Label>
          {draft.overall.momentum !== "none" && (
            <Label text={draft.overall.momentum.startsWith("percent") ? "Premium %" : "Points"}>
              <input className={control} type="number" min={0} value={draft.overall.momentum_value} onChange={(event) => patchOverall({ momentum_value: Number(event.target.value) })} />
            </Label>
          )}
          {(draft.overall.trailing === "lock_trail" || draft.overall.trailing === "overall_trail") && (
            <>
              <Label text="For every">
                <input className={control} type="number" value={draft.overall.trail_every} onChange={(event) => patchOverall({ trail_every: Number(event.target.value) })} />
              </Label>
              <Label text="Trail by">
                <input className={control} type="number" value={draft.overall.trail_by} onChange={(event) => patchOverall({ trail_by: Number(event.target.value) })} />
              </Label>
            </>
          )}
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button className="h-9 rounded-md px-3 text-sm text-slate-600 hover:bg-slate-100" type="button" onClick={onCancel}>
            Cancel
          </button>
          <button className="h-9 rounded-md bg-indigo-600 px-4 text-sm font-medium text-white hover:bg-indigo-500" type="button" onClick={onSave}>
            Save strategy
          </button>
        </div>
      </section>
    </div>
  );
}

function Label({ text, children }: { text: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-slate-500">
      {text}
      {children}
    </label>
  );
}
