# DESIGN.md — Solana Trading Terminal (paper-only)

> Source of truth for the visual language of the desktop terminal. Read this
> before touching any color / type / spacing / component change.
> Codified in `lib/theme.dart` and `lib/widgets/`.

Aesthetic direction adapted from public trading-terminal design systems —
most directly **Perpetua** ("terminal brutalist: dense, mono, hairlines,
Bloomberg-meets-Linear", https://github.com/tomiiide/perpetua),
**Inalpha Operator Console** ("Vermilion Terminal", https://github.com/mirror29/inalpha),
and **Zeno's Imperial Terminal** (https://github.com/ribeirogab/zeno-agent) —
plus the vstack `trading-design` skill guidelines
(https://github.com/vanillagreencom/vstack/tree/main/skills/trading-design).

## 1. Aesthetic Direction

**Terminal + control room.** A research desk for paper-trading agents on
Solana. Dense, scannable, utilitarian, dark-only. Every pixel carries data;
no decorative gradient, shadow, or rounded corner that does not earn its
space. It reads like a machine you can leave running overnight, not a SaaS
dashboard.

- Moody reference: Bloomberg / Sierra Chart density × Linear refinement ×
  terminal brass-and-cyan restraint.
- The interface is a **live agent workspace**: strategy run states, signals,
  simulated fills, risk events. Color is a priority channel, not decoration.

## 2. Color

Near-black cool surfaces, hairline borders only (no elevation/shadows), one
electric accent for data + interaction, semantic colors that carry the real
information (green-up / red-down convention, always).

### Surfaces

| Token | Hex | Use |
|---|---|---|
| `--bg` | `#08090F` | page canvas |
| `--bg-sidebar` | `#05060F` | sidebar |
| `--panel` | `#0F1119` | panels, cards |
| `--panel-2` | `#151824` | raised / hover / pressed |
| `--input` | `#0B0D16` | inputs, list rows |
| `--hairline` | `#1E2131` | 1px separators, borders |
| `--hairline-strong` | `#2A2E44` | strong borders, focus outline |

### Ink

| Token | Hex | Use |
|---|---|---|
| `--fg` | `#E8EAF5` | primary text, prices |
| `--fg-2` | `#8A8FAB` | secondary text |
| `--fg-3` | `#4B4F66` | labels, captions, disabled |
| `--fg-4` | `#232735` | ghost marks, placeholder |

### Semantic / accent

| Token | Hex | Use |
|---|---|---|
| `--accent` | `#4DD9C8` | data & interaction: links, focus, live values, charts, WS indicator |
| `--accent-strong` | `#14B8A6` | accent hover/pressed |
| `--up` | `#2FCF8E` | gain / buy / filled / running / qualified |
| `--down` | `#F0584B` | loss / sell / rejected / faulted / drawdown |
| `--warn` | `#E0B03F` | pending / paused / stale / needs attention |
| `--info` | `#7AA6E8` | info events, backtest queued |
| `--gold` | `#D9B362` | brand moments: the agent seal, "AGENT" mark, logo |

Rules:
- **Never** use `--gold`/`--accent` for market direction. Direction is always
  `--up` / `--down`. Accent is data, gold is identity.
- `--accent` appears sparingly: live value, focus ring, chart line, the live
  connection dot.
- Dim color usage by 60% for muted/per-chip states (e.g. `withValues(alpha:.6)`).

## 3. Typography

**Fonts:** JetBrains Mono (fallback chain: `Roboto Mono`, `Menlo`, `Consolas`,
`monospace`) for ALL numbers, tickers, timestamps, ids, status chips.
Inter / system sans for prose description text only.

Two deliberate conventions (from Perpetua):

1. **All financial figures are mono + tabular figures** (`FontFeature.tabularFigures()`)
   so columns align.
2. **Section labels, status chips, nav items: uppercase mono, wide tracking
   (`letterSpacing: 0.12–0.16em`), 10–11px.** Terse declaratives, no fluff.

Scale (12px base, compact):

| Role | Size / weight | Use |
|---|---|---|
| display | 24px / mono / w700 | page title (e.g. "SOLANA · PAPER TERMINAL") |
| hero | 20–22px / mono / w700 | equity hero number |
| panel title | 11px / mono / w700 / caps / 0.14em | panel headers ("EQUITY", "SIGNALS") |
| data | 12–13px / mono / w400–500 | all table values, prices |
| label | 10px / mono / caps / 0.12em | field labels, column headers |
| body | 12–13px / sans | strategy descriptions, notes |

## 4. Layout & Density

- 4px base grid; panel padding 8–12px; inter-element gaps 4–8px.
- Table row height 24–28px; data font 12px. A panel that scrolls for its core
  content has failed.
- Sidebar 200px, hairline-separated; content area scrolls horizontally-nothing,
  vertically per-panel.
- No shadows. Depth comes from surface steps + hairlines only.
- Radius: 0px (buttons/cards) to 4px max (chips, inputs). Sharp = terminal.

## 5. Components

- **Card/panel:** `--panel` surface, `--hairline` 1px border, 0 radii,
  panel title bar with caps tracking + trailing live-state chip (if any).
- **KPI:** label caps (fg-3) + hero mono value (fg). Delta chip: `--up`/`--down`
  with sign; muted 60% when zero.
- **Chip (status):** uppercase mono 10px, 1px border in the state color,
  dot (4px) before text; `paper_trading` → `--up`, `paused` → `--warn`,
  `faulted`/`disabled` → `--down`, `queued` → `--info`, `ready` → `--fg-3`.
- **Table:** header row = label caps fg-3; odd/even no stripes (hairline rows);
  numeric columns right-aligned mono; hover row `--panel-2`.
- **Buttons:** default = panel surface + hairline + fg; primary = accent
  surface w/ ink text (`--bg` text on `--accent`); danger = `--down` border,
  text --down. Caps mono 10px, 0.12em track. 0 radius. Height 28px.
- **Chart (equity):** line in `--accent`, area gradient accent→transparent 12%,
  grid `--hairline`; drawdown zone tinted `--down` 6%.
- **WS live dot:** 6px circle `--accent` pulse animation when connected;
  `--warn` idle when disconnected.

## 6. Do's & Don'ts

- Do: right-align every numeric column; prefix pct deltas with `+`/`-`; show
  timestamps mono; mark all simulated data (`SIM` chip on dashboard).
- Do: keep the sidebar ≤ 5 destinations; keyboard-first nav (1..5).
- Don't: gradients, glass blur, big radii, emoji in headings, uppercase
  prose (caps are for labels only), light mode, confetti/joy.
- Don't: color outside the semantic palette; never mix `--accent` into
  direction semantics.

## 7. Live states

| State | Color | Shown |
|---|---|---|
| connected (WS) | `--accent` pulsing dot | sidebar footer |
| backend down | `--down` | banner across top of content |
| provider stale | `--warn` | markets panel header chip |
| strategy paper_trading | `--up` chip | strategies card |
| backtest qualified=false | `--warn` + "DATA GAP" label | backtests row |