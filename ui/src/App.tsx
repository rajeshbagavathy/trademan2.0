import { useEffect, useState } from "react";
import { Builder } from "./builder";
import { Desk, PlainTable } from "./desk";
import { ActivateDialog, ExecutionDialog } from "./dialogs";
import { px } from "./lib/utils";
import { blankStrategy, type Definition, type Execution, type Snapshot, type Strategy } from "./types";

type Menu = "create" | "algo" | "orders" | "fills" | "audit";

const NAV: { id: Menu; label: string }[] = [
  { id: "create", label: "Create" },
  { id: "algo", label: "Algo trade" },
  { id: "orders", label: "Orders" },
  { id: "fills", label: "Fills" },
  { id: "audit", label: "Audit" },
];

export default function App() {
  const [menu, setMenu] = useState<Menu>("algo");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [building, setBuilding] = useState(false);
  const [draft, setDraft] = useState<Definition>(blankStrategy);
  const [openId, setOpenId] = useState<string | null>("nifty-22500-strangle");
  const [editing, setEditing] = useState<Strategy | null>(null);
  const [execution, setExecution] = useState<Execution | null>(null);
  const [activating, setActivating] = useState<Strategy | null>(null);
  const [broker, setBroker] = useState("Firstock");

  async function load() {
    const response = await fetch("/api/snapshot");
    if (response.ok) setSnap(await response.json());
  }

  useEffect(() => {
    void load();
    const timer = setInterval(() => void load(), 1000);
    return () => clearInterval(timer);
  }, []);

  async function post(path: string, body?: unknown) {
    await fetch(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body ?? {}),
    });
    await load();
  }

  const strategies = snap?.strategies ?? [];

  return (
    <div className="flex h-full min-h-screen flex-col">
      <header className="flex h-14 shrink-0 items-center gap-2 border-b border-slate-200 bg-white px-4">
        <div className="mr-2 flex items-center gap-2">
          <span className="grid h-7 w-7 place-items-center rounded-md bg-indigo-600 text-xs font-semibold text-white">T</span>
          <span className="text-sm font-semibold tracking-tight">Tradman</span>
        </div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className={`h-8 rounded-md px-3 text-sm ${menu === item.id && !building ? "bg-slate-900 text-white" : "text-slate-600 hover:bg-slate-100"}`}
            onClick={() => {
              setMenu(item.id);
              setBuilding(item.id === "create");
              if (item.id === "create") setDraft(blankStrategy());
            }}
          >
            {item.label}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-3 text-xs text-slate-500">
          <span className="rounded bg-slate-100 px-2 py-1 font-medium uppercase tracking-wide text-slate-600">{snap?.mode ?? "paper"}</span>
          {snap?.feeds.map((feed) => (
            <span key={feed.name} className="hidden items-center gap-1 md:flex">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
              {feed.name}
            </span>
          ))}
        </div>
      </header>

      <main className="flex min-h-0 flex-1 flex-col">
        {menu === "create" || building ? (
          <Builder
            draft={draft}
            onChange={setDraft}
            onCancel={() => {
              setBuilding(false);
              setMenu("algo");
            }}
            onSave={() => {
              void post("/api/strategies", draft).then(() => {
                setBuilding(false);
                setMenu("algo");
              });
            }}
          />
        ) : menu === "algo" ? (
          <Desk
            strategies={strategies}
            openId={openId}
            onOpen={(id) => setOpenId(openId === id ? null : id)}
            onEdit={(strategy) => {
              setEditing(strategy);
              setExecution(strategy.definition.execution);
            }}
            onActivate={(strategy) => {
              setActivating(strategy);
              setBroker(strategy.broker === "Not activated" ? "Firstock" : strategy.broker);
            }}
            onPause={(id) => void post(`/api/strategies/${id}/pause`)}
            onResume={(id) => void post(`/api/strategies/${id}/resume`)}
            onSquareOff={(id) => void post(`/api/strategies/${id}/square-off`)}
          />
        ) : menu === "orders" ? (
          <PlainTable
            headers={["Strategy", "Leg", "Role", "State", "Trigger", "Traded", "Qty"]}
            rows={(snap?.orders ?? []).map((order) => [order.strategy, order.leg, order.role, order.state, px(order.trigger), px(order.traded), String(order.qty)])}
          />
        ) : menu === "fills" ? (
          <PlainTable
            headers={["Strategy", "Leg", "Side", "Qty", "Price"]}
            rows={(snap?.fills ?? []).map((fill) => [fill.strategy, fill.leg, fill.side, String(fill.qty), px(fill.price)])}
          />
        ) : (
          <PlainTable headers={["Source", "Event"]} rows={(snap?.audit ?? []).map((row) => [row.source, row.event])} />
        )}
      </main>

      {editing && execution && (
        <ExecutionDialog
          execution={execution}
          onChange={setExecution}
          onClose={() => setEditing(null)}
          onSave={() => {
            void post(`/api/strategies/${editing.id}/execution`, execution).then(() => setEditing(null));
          }}
        />
      )}
      {activating && (
        <ActivateDialog
          broker={broker}
          onPick={setBroker}
          onClose={() => setActivating(null)}
          onActivate={() => {
            void post(`/api/strategies/${activating.id}/activate`, { broker }).then(() => setActivating(null));
          }}
        />
      )}
    </div>
  );
}
