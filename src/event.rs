use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};

use crate::error::Result;

/// All commands the application can execute in response to user input.
#[derive(Debug, Clone)]
pub enum AppCommand {
    Quit,
    NextView,
    PrevView,
    GoToDashboard,
    GoToTransactions,
    GoToStats,
    GoToBudgets,
    GoToRecurring,
    /// Open the add-transaction form.
    StartAddTransaction,
    /// Open the edit-transaction form for the selected transaction.
    StartEditTransaction,
    /// Delete the selected transaction (after confirmation).
    DeleteTransaction,
    /// Save the current transaction form (add or update).
    SaveTransaction,
    /// Cancel the current form/mode.
    Cancel,
    /// Move selection up in the current list.
    SelectUp,
    /// Move selection down in the current list.
    SelectDown,
    /// Open the filter prompt in the transactions view.
    StartFilter,
    /// Clear all active filters.
    ClearFilter,
    /// Toggle the selected recurring entry's active state.
    ToggleRecurring,
    /// Delete the selected recurring entry.
    DeleteRecurring,
    /// Add a new budget.
    StartAddBudget,
    /// Delete the selected budget.
    DeleteBudget,
    /// Tab to next field in form.
    NextField,
    /// Tab to previous field in form.
    PrevField,
    /// Toggle a boolean field in form (kind, recurring).
    ToggleField,
    /// Confirm a pending action.
    Confirm,
    /// Type a character into the current input field.
    TypeChar(char),
    /// Delete a character from the current input field (backspace).
    Backspace,
    /// Cycle through options in a selection field.
    CycleOption,
}

/// Events the application processes each tick of the main loop.
#[derive(Debug)]
pub enum AppEvent {
    /// A crossterm key event.
    Key(KeyEvent),
    /// A tick for periodic updates (status message timeout, etc.).
    Tick,
    /// Terminal resize.
    Resize(u16, u16),
}

/// Polls crossterm for events with a configurable tick rate.
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    /// Block until the next event is available or the tick rate elapses.
    pub fn next(&self) -> Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    // Only handle Press events to avoid double-firing on
                    // terminals that send both Press and Release.
                    if key.kind == KeyEventKind::Press {
                        Ok(AppEvent::Key(key))
                    } else {
                        Ok(AppEvent::Tick)
                    }
                }
                Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
