export type SlKind = "none" | "points" | "underlying_points" | "percent" | "underlying_percent" | "delta";
export type TrailKind = "none" | "points";
export type MoneyKind = "none" | "mtm" | "premium";
export type OverallTrail = "none" | "lock" | "lock_trail" | "overall_trail";
export type StrikeChoice = "atm" | "itm" | "otm";
export type LegMomentum = "none" | "percent_up" | "percent_down";
export type OverallMomentum = "none" | "points_up" | "points_down" | "percent_up" | "percent_down";

export type LegDraft = {
  position: "buy" | "sell";
  option_type: "CE" | "PE";
  expiry: "weekly" | "next" | "monthly" | "next_monthly";
  strike: StrikeChoice;
  strike_value: number;
  lots: number;
  sl: SlKind;
  sl_value: number;
  trail: TrailKind;
  trail_every: number;
  trail_by: number;
  target: SlKind;
  target_value: number;
  momentum: LegMomentum;
  momentum_value: number;
  range_breakout: "none" | "high" | "low" | "underlying" | "strike";
};

export type Overall = {
  target: MoneyKind;
  target_value: number;
  stop: MoneyKind;
  stop_value: number;
  trailing: OverallTrail;
  lock_at: number;
  lock_profit: number;
  trail_every: number;
  trail_by: number;
  momentum: OverallMomentum;
  momentum_value: number;
};

export type Execution = {
  multiplier: number;
  monitor: "ltp" | "candle-1" | "candle-3";
  product: "MIS" | "NRML";
  entry_order: "limit" | "mpp";
  exit_order: "limit" | "mpp";
  buffer_type: "points" | "percent";
  trigger_buffer: number;
  limit_buffer: number;
  ref_price: "traded" | "trigger";
  leg_monitoring: "continuous" | "delayed";
  frequency_seconds: number;
  auto_square_margin: boolean;
  delay_seconds: number;
  days: string[];
  dte: number[];
};

export type Definition = {
  name: string;
  underlying: string;
  underlying_from: "cash" | "futures";
  strategy_type: "intraday" | "btst" | "positional";
  entry_time: string;
  exit_time: string;
  square_off: "partial" | "complete";
  breakeven: "none" | "all" | "sl";
  legs: LegDraft[];
  overall: Overall;
  execution: Execution;
};

export type LiveLeg = {
  id: number;
  symbol: string;
  open: number;
  closed: number;
  entry: number | null;
  exit: number | null;
  trigger: number | null;
  realized: number;
  unrealized: number;
};

export type Strategy = {
  id: string;
  name: string;
  broker: string;
  state: string;
  monitor: string;
  combined: number;
  realized: number;
  unrealized: number;
  stop: number | null;
  product: string;
  starts_at: number | null;
  started_at: number | null;
  legs: LiveLeg[];
  definition: Definition;
};

export type Snapshot = {
  mode: string;
  feeds: { name: string; role: string }[];
  strategies: Strategy[];
  orders: { strategy: string; leg: string; role: string; state: string; trigger: number | null; traded: number | null; qty: number }[];
  fills: { strategy: string; leg: string; side: string; qty: number; price: number }[];
  audit: { source: string; event: string }[];
};

export const UNDERLYINGS = ["NIFTY", "BANKNIFTY", "FINNIFTY", "MIDCPNIFTY", "SENSEX"] as const;
export const WEEKDAYS = [
  ["mon", "Mon"],
  ["tue", "Tue"],
  ["wed", "Wed"],
  ["thu", "Thu"],
  ["fri", "Fri"],
] as const;

export function legDraft(option: "CE" | "PE"): LegDraft {
  return {
    position: "sell",
    option_type: option,
    expiry: "weekly",
    strike: "atm",
    strike_value: 0,
    lots: 1,
    sl: "percent",
    sl_value: 25,
    trail: "points",
    trail_every: 10,
    trail_by: 5,
    target: "none",
    target_value: 0,
    momentum: "none",
    momentum_value: 0,
    range_breakout: "none",
  };
}

export function blankStrategy(): Definition {
  return {
    name: "NIFTY weekly strangle",
    underlying: "NIFTY",
    underlying_from: "cash",
    strategy_type: "intraday",
    entry_time: "09:20",
    exit_time: "15:20",
    square_off: "complete",
    breakeven: "none",
    legs: [legDraft("CE"), legDraft("PE")],
    overall: {
      target: "none",
      target_value: 0,
      stop: "mtm",
      stop_value: 2000,
      trailing: "lock",
      lock_at: 4000,
      lock_profit: 1000,
      trail_every: 1000,
      trail_by: 500,
      momentum: "none",
      momentum_value: 0,
    },
    execution: {
      multiplier: 1,
      monitor: "candle-1",
      product: "MIS",
      entry_order: "limit",
      exit_order: "limit",
      buffer_type: "points",
      trigger_buffer: 0,
      limit_buffer: 0,
      ref_price: "traded",
      leg_monitoring: "continuous",
      frequency_seconds: 1,
      auto_square_margin: true,
      delay_seconds: 0,
      days: ["mon", "tue", "wed", "thu", "fri"],
      dte: [0, 1, 2, 3, 4],
    },
  };
}

export function strikeLabel(leg: LegDraft) {
  if (leg.strike === "itm") return "ITM";
  if (leg.strike === "otm") return "OTM";
  return "ATM";
}

export function legSummary(leg: LegDraft) {
  const side = leg.position === "buy" ? "Buy" : "Sell";
  return `${side} ${strikeLabel(leg)} ${leg.option_type}`;
}
