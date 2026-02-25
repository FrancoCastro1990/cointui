# CoinTUI

A terminal-based personal finance manager built with Rust, Ratatui, and SQLite.

Track income, expenses, budgets, and recurring transactions — all from the comfort of your terminal.

## Features

- **Dashboard** — At-a-glance view of your financial health: total income, expenses, balance, recent transactions, and budget alerts
- **Transaction management** — Add, edit, and delete income/expense records with source, amount, date, category, and notes
- **Hierarchical tags** — Organize transactions with categories and subcategories (e.g., Comida > Restaurantes)
- **Budgets with alerts** — Set spending limits per category or globally (weekly/monthly/yearly) with visual progress bars and warnings at 80%+ usage
- **Recurring transactions** — Define recurring entries (daily/weekly/monthly/yearly) that auto-insert on startup
- **Statistics** — Bar charts by category, monthly trends, income vs. expenses breakdown, and savings rate
- **Filtering** — Search transactions by text, date range, amount range, kind, and tag
- **CLI quick-add** — Add transactions directly from the command line without entering the TUI
- **CSV import** — Import transactions from CSV files with interactive column mapping
- **CSV/JSON export** — Export all transactions to CSV or JSON format
- **Database backup/restore** — Create and restore full database backups
- **Persistent storage** — SQLite database with WAL mode, automatic schema migrations, and safe parameterized queries
- **Locale-aware formatting** — Configurable thousands/decimal separators (default: Chilean format `$ 2.700.000,00`)
- **Configurable** — TOML config for currency symbol, number format, default tags, and database path

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
| `1` - `5` | Switch between views |
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
| `Space` | Toggle or cycle option fields (Kind, Tag, Recurring, Interval) |
| `Enter` | Save transaction |
| `Esc` | Cancel |

### Budgets

| Key | Action |
|-----|--------|
| `a` | Add new budget |
| `d` | Delete selected budget |

### Recurring entries

| Key | Action |
|-----|--------|
| `Space` | Toggle active/inactive |
| `d` | Delete entry |

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

Bar chart of expenses by category, monthly income/expense table for the last 6 months, total summary with savings rate.

### 4. Budgets

List of budget rules with gauge progress bars. Color indicators: green (< 60%), yellow (60-80%), red (> 80%).

### 5. Recurring

Manage recurring transaction templates. Toggle active/inactive, view interval and amounts.

## Configuration

Config file location: `~/.config/cointui/config.toml`

A default config is auto-created on first run:

```toml
currency = "$"
thousands_separator = "."
decimal_separator = ","
default_tags = [
    "Comida",
    "Transporte",
    "Entretenimiento",
    "Servicios",
    "Salario",
    "Salud",
    "Educación",
    "Otros",
]
```

### Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `currency` | String | `"$"` | Currency symbol displayed next to amounts |
| `thousands_separator` | String | `"."` | Thousands grouping separator (e.g. `"."` for `1.000`, `","` for `1,000`) |
| `decimal_separator` | String | `","` | Decimal separator (e.g. `","` for `1.000,50`, `"."` for `1,000.50`) |
| `default_tags` | Array | 8 categories | Tags seeded into a fresh database |
| `db_path` | String | (auto) | Override database file path |

## Data storage

- **Database**: SQLite at `~/.local/share/cointui/cointui.db`
- **Config**: TOML at `~/.config/cointui/config.toml`
- Amounts are stored in **centavos** (integer) to avoid floating-point errors
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
cargo test           # Run all tests (47 tests)
cargo clippy         # Lint (must pass with zero warnings)
cargo build --release
```

## Roadmap

- [ ] Help overlay (`?` key)
- [ ] Sortable columns in transaction table

## License

MIT
