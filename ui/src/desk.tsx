import { useEffect, useState } from "react";
import { Button } from "./components/ui/button";
import { inr, px } from "./lib/utils";
import { legSummary, type Strategy } from "./types";

function useNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

function clock(ms: number) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

export function Desk({
  strategies,
  openId,
  onOpen,
  onEdit,
  onActivate,
  onPause,
  onResume,
  onSquareOff,
}: {
  strategies: Strategy[];
  openId: string | null;
  onOpen: (id: string) => void;
  onEdit: (strategy: Strategy) => void;
  onActivate: (strategy: Strategy) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onSquareOff: (id: string) => void;
}) {
  const now = useNow();
  const saved = strategies.filter((strategy) => strategy.state === "Waiting");
  const queue = strategies
    .filter((strategy) => strategy.state !== "Waiting")
    .slice()
    .sort((left, right) => (left.definition?.entry_time ?? "").localeCompare(right.definition?.entry_time ?? ""));
  return (
    <div className="min-h-0 flex-1 space-y-8 overflow-auto p-4">
      <section>
        <h2 className="mb-3 text-sm font-semibold">Executions</h2>
        {queue.length === 0 ? (
          <div className="rounded-xl border border-dashed border-slate-200 bg-white px-4 py-8 text-center text-sm text-slate-500">No strategies in the queue. Activate one below and it waits here until its start time.</div>
        ) : (
          <div className="space-y-3">{queue.map((strategy) => card(strategy, true, now))}</div>
        )}
      </section>
      <section>
        <h2 className="mb-1 text-sm font-semibold">Activation</h2>
        <p className="mb-3 text-xs text-slate-500">Saved strategies. Activate one to queue it by its entry time.</p>
        {saved.length === 0 ? (
          <div className="rounded-xl border border-dashed border-slate-200 bg-white px-4 py-8 text-center text-sm text-slate-500">No saved strategies. Create one, then it shows up here.</div>
        ) : (
          <div className="space-y-3">{saved.map((strategy) => card(strategy, false, now))}</div>
        )}
      </section>
    </div>
  );

  function card(strategy: Strategy, queued: boolean, nowMs: number) {
        const open = strategy.id === openId;
        const parked = strategy.state === "Waiting" || strategy.state === "Flat";
        const definition = strategy.definition;
        const running = strategy.started_at != null;
        const remaining = strategy.starts_at == null ? 0 : strategy.starts_at - nowMs;
        return (
          <article key={strategy.id} className="rounded-xl border border-slate-200 bg-white">
            <div className="flex flex-wrap items-center gap-4 px-4 py-3">
              <button className="min-w-0 flex-1 text-left" type="button" onClick={() => onOpen(strategy.id)}>
                <div className="flex items-center gap-2">
                  <h2 className="truncate text-sm font-semibold">{definition?.name || strategy.name}</h2>
                  <Badge state={strategy.state} />
                </div>
                <p className="mt-1 truncate text-xs text-slate-500">
                  {definition?.underlying} · {definition?.strategy_type} · DTE {(definition?.execution.dte ?? []).join(", ") || "—"} · {definition?.legs.map(legSummary).join(" · ")}
                </p>
              </button>
              {queued ? (
                <div className="min-w-24 text-right">
                  <div className="text-[11px] uppercase tracking-wide text-slate-400">{running ? "Running" : "Starts in"}</div>
                  <div className="font-mono text-lg font-semibold tabular-nums text-slate-900">{clock(running ? nowMs - (strategy.started_at ?? nowMs) : remaining)}</div>
                  <div className="text-[11px] text-slate-400">Entry {definition?.entry_time ?? "—"}</div>
                </div>
              ) : (
                <div className="text-right text-xs text-slate-500">Entry {definition?.entry_time ?? "—"}</div>
              )}
              <div className="text-right">
                <div className="text-[11px] uppercase tracking-wide text-slate-400">Combined</div>
                <Pnl value={strategy.combined} />
              </div>
              <div className="flex flex-wrap justify-end gap-2">
                <Button onClick={() => onEdit(strategy)}>Edit execution</Button>
                {!queued && <Button tone="primary" onClick={() => onActivate(strategy)}>Activate</Button>}
                {queued && strategy.state === "Paused" && <Button onClick={() => onResume(strategy.id)}>Resume</Button>}
                {queued && !parked && strategy.state !== "Paused" && <Button onClick={() => onPause(strategy.id)}>Pause</Button>}
                {queued && !parked && <Button onClick={() => onSquareOff(strategy.id)}>Square off</Button>}
              </div>
            </div>
            {open && (
              <div className="border-t border-slate-100 px-4 py-3">
                <div className="mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-slate-500">
                  <span>Broker {strategy.broker}</span>
                  <span>{strategy.monitor}</span>
                  <span>{strategy.product}</span>
                  <span>Realized <Pnl value={strategy.realized} small /></span>
                  <span>Unrealized <Pnl value={strategy.unrealized} small /></span>
                  <span>Overall stop {strategy.stop == null ? "—" : inr(strategy.stop)}</span>
                </div>
                <table className="w-full text-sm tabular-nums">
                  <thead className="text-left text-xs text-slate-400">
                    <tr>
                      {["Leg", "Open", "Closed", "Entry", "Exit", "Stop trigger", "Realized", "Unrealized"].map((label) => (
                        <th key={label} className="py-1 pr-3 font-medium">{label}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {strategy.legs.map((leg, index) => (
                      <tr key={leg.id} className="border-t border-slate-100">
                        <td className="py-2 pr-3">
                          <div>{leg.symbol}</div>
                          <div className="text-xs text-slate-400">{ruleText(definition?.legs[index])}</div>
                        </td>
                        <td className="pr-3">{leg.open}</td>
                        <td className="pr-3">{leg.closed}</td>
                        <td className="pr-3">{px(leg.entry)}</td>
                        <td className="pr-3">{px(leg.exit)}</td>
                        <td className="pr-3">{px(leg.trigger)}</td>
                        <td className="pr-3"><Pnl value={leg.realized} small /></td>
                        <td><Pnl value={leg.unrealized} small /></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </article>
        );
  }
}

function ruleText(leg: Strategy["definition"]["legs"][number] | undefined) {
  if (!leg) return "";
  const sl = leg.sl === "none" ? "No SL" : `SL ${leg.sl.replaceAll("_", " ")} ${leg.sl_value}`;
  const trail = leg.trail === "none" ? "no trail" : `trail ${leg.trail_every}/${leg.trail_by} ${leg.trail}`;
  return `${sl} · ${trail}`;
}

function Badge({ state }: { state: string }) {
  const tone =
    state === "Live" ? "bg-emerald-50 text-emerald-700" :
    state === "Waiting" ? "bg-amber-50 text-amber-700" :
    state === "Queued" ? "bg-indigo-50 text-indigo-700" :
    state === "Paused" ? "bg-slate-100 text-slate-600" :
    "bg-indigo-50 text-indigo-700";
  const label = state === "Waiting" ? "Saved" : state;
  return <span className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${tone}`}>{label}</span>;
}

export function Pnl({ value, small }: { value: number; small?: boolean }) {
  const tone = value > 0 ? "text-emerald-600" : value < 0 ? "text-rose-600" : "text-slate-700";
  return <span className={`${tone} ${small ? "" : "text-base font-semibold"} tabular-nums`}>{inr(value)}</span>;
}

export function PlainTable({ headers, rows }: { headers: string[]; rows: string[][] }) {
  return (
    <div className="min-h-0 flex-1 overflow-auto p-4">
      <table className="w-full overflow-hidden rounded-xl border border-slate-200 bg-white text-sm tabular-nums">
        <thead className="text-left text-xs text-slate-400">
          <tr>{headers.map((header) => <th key={header} className="border-b border-slate-100 px-3 py-2 font-medium">{header}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={index} className="border-b border-slate-50">
              {row.map((cell, cellIndex) => <td key={cellIndex} className="px-3 py-2">{cell}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
