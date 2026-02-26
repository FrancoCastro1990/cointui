# CoinTUI

A terminal-based personal finance manager built with Rust, Ratatui, and SQLite.

Track income, expenses, budgets, and recurring transactions — all from the comfort of your terminal.

## Features

- **Dashboard** — At-a-glance view of your financial health: total income, expenses, balance, recent transactions, and budget alerts
- **Transaction management** — Add, edit, and delete income/expense records with source, amount, date, category, and notes
- **Hierarchical tags** — Organize transactions with categories and subcategories (e.g., Comida > Restaurantes)
- **Budgets with alerts** — Set spending limits per category or globally (weekly/monthly/yearly) with visual progress bars and warnings at 80%+ usage
- **Recurring transactions** — Full CRUD for recurring entries (daily/weekly/monthly/yearly) with configurable day-of-month and month; auto-insert on startup
- **Statistics** — Three sub-tabs (Overview, Trends, Budgets) with totals, savings rate gauge, expense breakdown by tag, monthly trends with configurable range (6/12/24 months), and budget status gauges
- **Filtering** — Search transactions by text, date range, amount range, kind, and tag
- **Tag management** — Create, rename, and delete tags from CLI or TUI with safe reassignment when tags are in use
- **CLI quick-add** — Add transactions directly from the command line without entering the TUI
- **CSV import** — Import transactions from CSV files with interactive column mapping
- **CSV/JSON export** — Export all transactions to CSV or JSON format
- **Database backup/restore** — Create and restore full database backups
- **Persistent storage** — SQLite database with WAL mode, automatic schema migrations, and safe parameterized queries
- **Locale-aware formatting** — Configurable thousands/decimal separators (default: Chilean format `$ 2.700.000,00`)
- **Configurable** — TOML config for currency symbol, number format, and database path

## Installation

### From source

```bash
git clone https://github.com/tu-usuario/cointui.git
cd cointui
cargo build --release
```

The binary will be at `target/release/cointui`.

### Requirements

- Rust 2024 edition (1.85+)
- No external dependencies — SQLite is bundled via `rusqlite`

## Usage

```bash
# Launch the TUI
cointui

# Use a custom config file
cointui --config /path/to/config.toml
```

### Quick-add transactions from the CLI

```bash
# Minimal (defaults: kind=expense, date=today, tag=Otros)
cointui --add "Supermercado" --amount 50.00

# Full options
cointui --add "Supermercado" --amount 50.00 \
  --tag Comida --kind expense \
  --date 2026-02-25 --notes "weekly groceries"

# Income
cointui --add "Salario" --amount 3000 --kind income --tag Salario
```

### Tag management

```bash
# List all tags
cointui --tags

# Add a new tag
cointui --add-tag "Food"

# Rename a tag
cointui --rename-tag "Food:Groceries"

# Delete a tag (blocks if transactions or recurring entries reference it)
cointui --delete-tag "Groceries"
```

### Import / Export

```bash
# Import from CSV (interactive column mapping)
cointui --import transactions.csv

# Export to JSON (format detected from extension)
cointui --export transactions.json

# Export to CSV
cointui --export transactions.csv

# Force a specific format
cointui --export data.txt --format json
```

### Backup / Restore

```bash
# Create a backup (auto-generated timestamped filename)
cointui --backup

# Create a backup at a specific path
cointui --backup /path/to/backup.db

# Restore from a backup
cointui --restore /path/to/backup.db
```

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1` - `6` | Switch between views |
| `Tab` / `Shift+Tab` | Cycle views |
| `q` | Quit |
| `Esc` | Return to Dashboard |

### Transaction list

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `a` | Add new transaction |
| `e` | Edit selected transaction |
| `d` | Delete selected transaction |
| `/` | Filter by text |

### Transaction form

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field |
| `Space` | Toggle or cycle option fields (Kind, Tag) |
| `Enter` | Save transaction |
| `Esc` | Cancel |

### Budgets

| Key | Action |
|-----|--------|
| `a` | Add new budget |
| `e` | Edit selected budget |
| `d` | Delete selected budget |

### Stats

| Key | Action |
|-----|--------|
| `h` / `Left` | Previous sub-tab |
| `l` / `Right` | Next sub-tab |
| `m` | Cycle month range (6 / 12 / 24) |

### Recurring entries

| Key | Action |
|-----|--------|
| `a` | Add new recurring entry |
| `e` | Edit selected entry |
| `Space` | Toggle active/inactive |
| `d` | Delete entry |

### Tags

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `a` | Add new tag |
| `e` | Edit selected tag |
| `d` | Delete selected tag |

## Views

### 1. Dashboard

```
┌─ INCOME ──────┐┌─ BALANCE ─────┐┌─ EXPENSES ────┐
│ $ 3.500.000,00 ││ $ 1.200.000,00 ││ $ 2.300.000,00 │
└────────────────┘└────────────────┘└────────────────┘
┌─ Recent Transactions ─────────────────────────────┐
│ Date       Source          Amount    Type    Tag   │
│ 2026-02-25 Supermercado  $ 45.000,00 EXP   Comida │
│ 2026-02-01 Salario    $ 2.700.000,00 INC   Salario│
└───────────────────────────────────────────────────┘
┌─ Budget Alerts ───────────────────────────────────┐
│ ⚠ Comida: 85% used ($ 425.000,00 / $ 500.000,00) │
└───────────────────────────────────────────────────┘
```

### 2. Transactions

Full scrollable list with all transactions, filterable by text, date, amount, kind, and tag. Color-coded amounts (green for income, red for expenses).

### 3. Stats

Three sub-tabs navigable with `h`/`l`:

- **Overview** — Income/Balance/Expenses header, savings rate gauge, and expense breakdown by tag with horizontal bars and percentages
- **Trends** — Monthly income vs. expenses with line gauges, configurable range (6/12/24 months via `m` key)
- **Budgets** — Budget progress gauges with on-track/warning/over-budget summary

### 4. Budgets

List of budget rules with gauge progress bars. Color indicators: green (< 60%), yellow (60-80%), red (> 80%).

### 5. Recurring

Full CRUD for recurring transaction templates. Add, edit, toggle active/inactive, and delete entries. Monthly entries let you pick the day of month (1-31), yearly entries let you pick month and day. Schedule is shown in the Interval column (e.g. "Monthly (15)", "Yearly (Mar 15)").

### 6. Tags

Manage category tags. Add, rename, and delete tags. When deleting a tag that has transactions or recurring entries, a reassignment modal lets you pick which tag to move them to before deletion.

## Configuration

Config file location: `~/.config/cointui/config.toml`

A default config is auto-created on first run:

```toml
currency = "$"
thousands_separator = "."
decimal_separator = ","
```

### Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `currency` | String | `"$"` | Currency symbol displayed next to amounts |
| `thousands_separator` | String | `"."` | Thousands grouping separator (e.g. `"."` for `1.000`, `","` for `1,000`) |
| `decimal_separator` | String | `","` | Decimal separator (e.g. `","` for `1.000,50`, `"."` for `1,000.50`) |
| `db_path` | String | (auto) | Override database file path |

## Data storage

- **Database**: SQLite at `~/.local/share/cointui/cointui.db`
- **Config**: TOML at `~/.config/cointui/config.toml`
- Amounts are stored as **whole currency units** (integer) to avoid floating-point errors
- WAL journal mode enabled for performance
- Foreign keys enforced

### Schema

Four tables: `tags`, `transactions`, `budgets`, `recurring_entries`. See `src/db/connection.rs` for the full schema.

## Architecture

```
src/
├── main.rs              # Entry point, CLI flag dispatch, terminal setup, event loop
├── app.rs               # App state machine (View, Mode, command dispatch)
├── event.rs             # Event bus (crossterm events + tick)
├── error.rs             # Error types (thiserror)
├── config.rs            # TOML configuration
├── cli/
│   ├── add.rs           # --add transaction from CLI
│   ├── tags.rs          # --tags, --add-tag, --rename-tag, --delete-tag
│   ├── import.rs        # --import CSV with column mapping
│   ├── export.rs        # --export to CSV/JSON
│   └── backup.rs        # --backup / --restore
├── domain/
│   └── models.rs        # Transaction, Tag, Budget, RecurringEntry
├── db/
│   ├── connection.rs    # SQLite connection, schema, migrations
│   ├── transaction_repo.rs  # Transaction CRUD + filtering
│   ├── tag_repo.rs      # Tag CRUD (hierarchical)
│   ├── budget_repo.rs   # Budget CRUD + spent calculation
│   └── recurring_repo.rs    # Recurring entry CRUD
└── ui/
    ├── theme.rs         # Tokyo Night color palette
    └── views/
        ├── dashboard.rs # Balance header, recent transactions, alerts
        ├── transactions.rs  # Full transaction list
        ├── form.rs      # Add/edit transaction popup form
        ├── filter_form.rs   # Advanced filter popup form
        ├── stats.rs     # Charts and financial summaries
        ├── budget.rs    # Budget list with progress gauges
        ├── recurring.rs # Recurring entry management
        ├── tags.rs      # Tag management with add/edit/delete
        └── help.rs      # Help overlay
```

**Layered architecture**: CLI / UI -> Event Bus -> App State -> Repository (SQLite)

## Tech stack

| Crate | Version | Purpose |
|-------|---------|---------|
| [ratatui](https://ratatui.rs) | 0.29 | Terminal UI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | 0.28 | Terminal backend |
| [rusqlite](https://github.com/rusqlite/rusqlite) | 0.32 | SQLite (bundled) |
| [chrono](https://github.com/chronotope/chrono) | 0.4 | Date/time handling |
| [clap](https://github.com/clap-rs/clap) | 4 | CLI argument parsing |
| [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | 1 / 0.8 | Config serialization |
| [serde_json](https://github.com/serde-rs/json) | 1 | JSON export |
| [thiserror](https://github.com/dtolnay/thiserror) | 2 | Error derivation |
| [csv](https://github.com/BurntSushi/rust-csv) | 1.3 | CSV import/export |
| [directories](https://github.com/dirs-dev/directories-rs) | 6 | XDG paths |

## Development

```bash
cargo check          # Fast compilation check
cargo test           # Run all tests (62 tests)
cargo clippy         # Lint (must pass with zero warnings)
cargo build --release
```

## Roadmap

- [x] Help overlay (`?` key)
- [x] Sortable columns in transaction table
- [x] Tag management (CLI + TUI)
- [x] Stats redesign with sub-tabs (Overview, Trends, Budgets)
- [x] Recurring CRUD with configurable intervals (day of month, month)

## License

MIT
