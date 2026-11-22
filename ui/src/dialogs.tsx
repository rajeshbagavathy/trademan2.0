import type { ReactNode } from "react";
import { WEEKDAYS, type Execution } from "./types";
import { Button } from "./components/ui/button";

const BROKERS = [
  { name: "Firstock", detail: "One session. Market, orders, and positions on the same socket." },
  { name: "5paisa", detail: "Symphony XTS. Separate market-data and interactive sessions." },
];

export function ExecutionDialog({
  execution,
  onChange,
  onClose,
  onSave,
}: {
  execution: Execution;
  onChange: (next: Execution) => void;
  onClose: () => void;
  onSave: () => void;
}) {
  function toggleDay(day: string) {
    const days = execution.days.includes(day) ? execution.days.filter((item) => item !== day) : [...execution.days, day];
    onChange({ ...execution, days });
  }
  function toggleDte(day: number) {
    const dte = execution.dte.includes(day) ? execution.dte.filter((item) => item !== day) : [...execution.dte, day].sort((a, b) => a - b);
    onChange({ ...execution, dte });
  }

  return (
    <Modal title="Edit execution" onClose={onClose}>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Quantity multiplier">
          <input className="field" type="number" min={1} value={execution.multiplier} onChange={(event) => onChange({ ...execution, multiplier: Number(event.target.value) })} />
        </Field>
        <Field label="Trade monitoring">
          <select className="field" value={execution.monitor} onChange={(event) => onChange({ ...execution, monitor: event.target.value as Execution["monitor"] })}>
            <option value="ltp">LTP</option>
            <option value="candle-1">Candle close 1 min</option>
            <option value="candle-3">Candle close 3 min</option>
          </select>
        </Field>
        <Field label="Product">
          <select className="field" value={execution.product} onChange={(event) => onChange({ ...execution, product: event.target.value as Execution["product"] })}>
            <option value="MIS">MIS</option>
            <option value="NRML">NRML</option>
          </select>
        </Field>
        <Field label="Delay entry by (seconds)">
          <input className="field" type="number" min={0} value={execution.delay_seconds} onChange={(event) => onChange({ ...execution, delay_seconds: Number(event.target.value) })} />
        </Field>
        <Field label="Entry order">
          <select className="field" value={execution.entry_order} onChange={(event) => onChange({ ...execution, entry_order: event.target.value as Execution["entry_order"] })}>
            <option value="limit">Limit</option>
            <option value="mpp">MPP</option>
          </select>
        </Field>
        <Field label="Exit order">
          <select className="field" value={execution.exit_order} onChange={(event) => onChange({ ...execution, exit_order: event.target.value as Execution["exit_order"] })}>
            <option value="limit">Limit</option>
            <option value="mpp">MPP</option>
          </select>
        </Field>
        <Field label="Tgt / SL reference">
          <select className="field" value={execution.ref_price} onChange={(event) => onChange({ ...execution, ref_price: event.target.value as Execution["ref_price"] })}>
            <option value="traded">Traded price</option>
            <option value="trigger">Trigger price</option>
          </select>
        </Field>
        <Field label="Buffer type">
          <select className="field" value={execution.buffer_type} onChange={(event) => onChange({ ...execution, buffer_type: event.target.value as Execution["buffer_type"] })}>
            <option value="points">Points</option>
            <option value="percent">Percent</option>
          </select>
        </Field>
        <Field label="Trigger buffer">
          <input className="field" type="number" value={execution.trigger_buffer} onChange={(event) => onChange({ ...execution, trigger_buffer: Number(event.target.value) })} />
        </Field>
        <Field label="Limit buffer">
          <input className="field" type="number" value={execution.limit_buffer} onChange={(event) => onChange({ ...execution, limit_buffer: Number(event.target.value) })} />
        </Field>
        <Field label="Leg monitoring">
          <select className="field" value={execution.leg_monitoring} onChange={(event) => onChange({ ...execution, leg_monitoring: event.target.value as Execution["leg_monitoring"] })}>
            <option value="continuous">Continuous</option>
            <option value="delayed">Delayed</option>
          </select>
        </Field>
        {execution.leg_monitoring === "delayed" && (
          <Field label="Frequency (seconds)">
            <input className="field" type="number" min={1} value={execution.frequency_seconds} onChange={(event) => onChange({ ...execution, frequency_seconds: Number(event.target.value) })} />
          </Field>
        )}
        <label className="flex items-center gap-2 text-xs font-medium text-slate-600 sm:col-span-2">
          <input type="checkbox" checked={execution.auto_square_margin} onChange={(event) => onChange({ ...execution, auto_square_margin: event.target.checked })} />
          Auto square-off on margin error
        </label>
      </div>
      <div className="mt-4">
        <div className="text-xs font-medium text-slate-500">Trading days</div>
        <div className="mt-2 flex gap-2">
          {WEEKDAYS.map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`h-8 rounded-md px-2.5 text-xs font-medium ${execution.days.includes(id) ? "bg-indigo-600 text-white" : "bg-slate-100 text-slate-600"}`}
              onClick={() => toggleDay(id)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      <div className="mt-4 border-t border-slate-100 pt-4">
        <div className="text-xs font-medium text-slate-500">Days to expiry</div>
        <p className="mt-1 text-xs text-slate-400">New entries are taken only on the selected DTE values.</p>
        <div className="mt-2 flex flex-wrap gap-2">
          {[0, 1, 2, 3, 4, 5, 6].map((day) => (
            <button
              key={day}
              type="button"
              className={`h-8 w-8 rounded-md text-xs font-medium ${execution.dte.includes(day) ? "bg-slate-900 text-white" : "bg-slate-100 text-slate-600"}`}
              onClick={() => toggleDte(day)}
            >
              {day}
            </button>
          ))}
        </div>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button type="button" onClick={onClose}>Close</Button>
        <Button tone="primary" type="button" onClick={onSave}>Save execution</Button>
      </div>
    </Modal>
  );
}

export function ActivateDialog({
  broker,
  onPick,
  onClose,
  onActivate,
}: {
  broker: string;
  onPick: (broker: string) => void;
  onClose: () => void;
  onActivate: () => void;
}) {
  return (
    <Modal title="Activate strategy" onClose={onClose}>
      <p className="text-sm text-slate-500">Pick the broker account. The engine places the legs. This screen does not send a raw order.</p>
      <div className="mt-4 grid gap-2">
        {BROKERS.map((item) => (
          <button
            key={item.name}
            type="button"
            className={`rounded-lg border px-3 py-3 text-left ${broker === item.name ? "border-indigo-500 bg-indigo-50" : "border-slate-200 hover:bg-slate-50"}`}
            onClick={() => onPick(item.name)}
          >
            <div className="text-sm font-semibold">{item.name}</div>
            <div className="mt-0.5 text-xs text-slate-500">{item.detail}</div>
          </button>
        ))}
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button type="button" onClick={onClose}>Close</Button>
        <Button tone="primary" type="button" disabled={!broker} onClick={onActivate}>Activate</Button>
      </div>
    </Modal>
  );
}

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-slate-900/40 p-4" onClick={onClose}>
      <div className="max-h-[90vh] w-full max-w-xl overflow-auto rounded-xl bg-white p-5 shadow-xl" onClick={(event) => event.stopPropagation()}>
        <h2 className="text-base font-semibold">{title}</h2>
        <div className="mt-4">{children}</div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-slate-500">
      {label}
      {children}
    </label>
  );
}
