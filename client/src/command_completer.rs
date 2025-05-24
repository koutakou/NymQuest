use rustyline::{Context, Helper, Result};
use std::borrow::Cow::{self, Borrowed};

/// Custom history hinter to provide command suggestions
pub struct GameHistoryHinter {
    hinter: rustyline::hint::HistoryHinter,
}

impl GameHistoryHinter {
    /// Create a new GameHistoryHinter
    pub fn new() -> Self {
        Self {
            hinter: rustyline::hint::HistoryHinter {},
        }
    }
}

// Implement all required traits for the Helper trait

impl rustyline::completion::Completer for GameHistoryHinter {
    type Candidate = rustyline::completion::Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        // Command completion functionality
        let mut completions = Vec::new();

        // Only provide completions for commands that start with / or are at beginning
        if line.starts_with('/') || line.is_empty() {
            let partial = if let Some(stripped) = line.strip_prefix('/') {
                stripped
            } else {
                line
            };
            let _pos_offset = if line.starts_with('/') { 1 } else { 0 };

            // Full command list for auto-completion
            let commands = [
                "register", "r", "move", "m", "go", "attack", "a", "chat", "c", "say", "help", "h",
                "?", "quit", "q", "exit", "up", "u", "north", "n", "down", "d", "south", "s",
                "left", "l", "west", "w", "right", "ri", "east", "e", "ne", "nw", "se", "sw",
            ];

            for &cmd in &commands {
                if cmd.starts_with(partial) {
                    let display = if line.starts_with('/') {
                        format!("/{}\t", cmd)
                    } else {
                        cmd.to_string()
                    };
                    completions.push(rustyline::completion::Pair {
                        display,
                        replacement: cmd.to_string(),
                    });
                }
            }

            return Ok((pos - partial.len(), completions));
        }

        Ok((pos, completions))
    }
}

impl rustyline::hint::Hinter for GameHistoryHinter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // First try to get a hint from command history
        if let Some(hint) = self.hinter.hint(line, pos, ctx) {
            return Some(hint);
        }

        // If no history hint, provide suggestions for common commands
        if line.starts_with('/') || line.is_empty() {
            let partial = if let Some(stripped) = line.strip_prefix('/') {
                stripped
            } else {
                line
            };

            // Command suggestions based on what user has typed
            let suggestions = [
                ("r", "register"),
                ("m", "move"),
                ("a", "attack"),
                ("c", "chat"),
                ("h", "help"),
                ("q", "quit"),
                // Movement shortcuts
                ("u", "up"),
                ("d", "down"),
                ("l", "left"),
                ("ri", "right"),
                ("n", "north"),
                ("s", "south"),
                ("e", "east"),
                ("w", "west"),
            ];

            for (abbrev, full) in suggestions.iter() {
                if full.starts_with(partial) && partial != *full {
                    // Return the remainder of the full command as a hint
                    return Some(full[partial.len()..].to_string());
                }

                if abbrev.starts_with(partial) && partial != *abbrev {
                    // Return the remainder of the abbreviated command as a hint
                    return Some(abbrev[partial.len()..].to_string());
                }
            }
        }

        None
    }
}

impl rustyline::highlight::Highlighter for GameHistoryHinter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // No special highlighting needed
        Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        false
    }
}

impl rustyline::validate::Validator for GameHistoryHinter {
    fn validate(
        &self,
        _ctx: &mut rustyline::validate::ValidationContext,
    ) -> Result<rustyline::validate::ValidationResult> {
        // All input is valid for our case
        Ok(rustyline::validate::ValidationResult::Valid(None))
    }
}

// Implement the Helper trait which combines all the above traits
impl Helper for GameHistoryHinter {}
