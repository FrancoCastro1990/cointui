# Stats View Redesign — Design Document

**Date:** 2026-02-25
**Goal:** Improve Stats view with professional charts, MoM comparisons, time filtering, and budget projections. No new dependencies — uses Ratatui 0.29 built-in widgets (BarChart, BarGroup, Sparkline, LineGauge).

## Approach

Enhance existing 3-tab structure (Overview | Trends | Budgets) with richer data and better visualizations using Ratatui's built-in `BarChart::grouped`, `Sparkline`, and `LineGauge` widgets.

## Tab 1: Overview

### Layout
```
[Period indicator: Monthly/Yearly toggle — press 'm']
[Summary cards: Income | Balance | Expenses — with MoM/YoY delta]
[Savings Rate LineGauge]
[Expense by Tag — horizontal bars with percentages]
```

### Changes from current
- **Time filter**: `m` toggles between Monthly (current month) and Yearly (current year) views
- **MoM/YoY deltas**: Each summary card shows `▲ +X,X% vs <prev period>` colored green/red
- **Period-scoped data**: All totals, savings rate, and expense breakdown reflect the selected period
- Delta calculation: `((current - previous) / previous) * 100%`

### New data requirements
- `get_totals_for_period(start_date, end_date)` — filtered totals for current and previous period
- `get_expense_by_tag_for_period(start_date, end_date)` — tag breakdown for selected period

### New App state
- `stats_overview_period: OverviewPeriod` enum (Monthly, Yearly)

## Tab 2: Trends

### Layout
```
[Grouped BarChart: Income vs Expenses per month — 6/12/24 months]
[Monthly detail table with columns: Month | Income | Expense | Net | MoM Δ]
[Averages footer row]
```

### Changes from current
- **Grouped BarChart** replaces the per-month LineGauge rows. Uses `BarChart::grouped` with `BarGroup` per month (green=Income, red=Expense)
- **Monthly table** below the chart with MoM delta column (% change in net vs previous month)
- **Averages row** at table bottom showing mean income, expense, and net over the range
- `m` cycles range (6 → 12 → 24 months), updates both chart and table

### BarChart implementation
- Each month = one `BarGroup` with 2 `Bar` items (Income styled green, Expense styled red)
- `bar_width` adapts to available width and number of months
- Labels show abbreviated month names (e.g., "Sep", "Oct")
- Values hidden on bars (shown in table below)

### MoM delta calculation
- Per month: `net_delta = ((net_current - net_previous) / |net_previous|) * 100%`
- Displayed as `▲ +X,X%` (green) or `▼ -X,X%` (red)

## Tab 3: Budgets

### Layout
```
[Per budget: name/period + spent/limit + percentage + LineGauge + pace projection]
[Summary: on_track / warning / over counts]
```

### Changes from current
- **Pace projection line** added per budget: "Pace: $X projected by <end of period>"
- Projection calculation: `(spent / days_elapsed) * total_days_in_period`
- Projection status:
  - `projected < limit`: green "On track"
  - `projected >= limit * 0.8 && projected < limit`: yellow "Warning"
  - `projected >= limit`: red "OVER BUDGET"

### New data requirements
- Period start/end date calculation for each budget (already partially exists in BudgetRepo)
- Days elapsed / total days computation

## Data Layer Changes

### New TransactionRepo methods
- `get_totals_for_period(start: &str, end: &str) -> Result<(i64, i64)>` — sum income/expense between dates
- `get_expense_by_tag_for_period(start: &str, end: &str) -> Result<Vec<(i64, i64)>>` — tag-grouped expenses for period

### New App state fields
- `stats_overview_period: OverviewPeriod` — Monthly or Yearly
- `overview_totals: (i64, i64)` — period-scoped totals
- `overview_prev_totals: (i64, i64)` — previous period totals (for delta)
- `overview_expense_by_tag: Vec<(i64, i64)>` — period-scoped tag breakdown

### Budget pace calculation
- Computed at render time in the view (no new state needed)
- Uses `chrono::Local::now()` for current date and budget period boundaries

## Key Bindings

| Key | Context | Action |
|-----|---------|--------|
| `m` | Overview | Toggle Monthly/Yearly period |
| `m` | Trends | Cycle 6/12/24 months range |
| `h`/`l` or arrows | All tabs | Switch sub-tab |

## Dependencies

None added. All widgets are built into Ratatui 0.29. `chrono` is already a dependency for date handling.

## Testing

- Unit tests for new repo methods with in-memory DB
- Unit tests for delta percentage calculation
- Unit tests for pace projection math
- Existing tests must continue passing
