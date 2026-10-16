//! Application state, configuration, and explicit state machine for idle monitoring.

use crate::config::Config;
use crate::logger;

/// Menu item command IDs for the native tray context menu.
/// Starts at 1001 as required by specifications.
pub const IDM_OPEN_SETTINGS: u32 = 1001;
pub const IDM_EXIT: u32 = 1002;

/// Parsed tray menu user actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// No action (e.g. menu dismissed by clicking away or unknown ID).
    None,
    OpenSettings,
    Exit,
}

/// Pure function mapping Win32 menu command IDs to high-level [`MenuAction`]s.
///
/// Guarantees that ID 0 (dismissed menu) and unknown IDs map strictly to [`MenuAction::None`],
/// never accidentally triggering [`MenuAction::Exit`].
pub fn map_menu_id(id: u32) -> MenuAction {
    match id {
        IDM_OPEN_SETTINGS => MenuAction::OpenSettings,
        IDM_EXIT => MenuAction::Exit,
        _ => MenuAction::None,
    }
}

/// Selectable idle timeout intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleTimeout {
    FiveMinutes = 5,
    FifteenMinutes = 15,
    ThirtyMinutes = 30,
    OneHour = 60,
    TwoHours = 120,
}

impl IdleTimeout {
    /// All available timeout intervals in ascending order.
    pub const ALL: [Self; 5] = [
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::ThirtyMinutes,
        Self::OneHour,
        Self::TwoHours,
    ];

    /// Returns the timeout duration in minutes.
    pub const fn as_minutes(&self) -> u32 {
        *self as u32
    }

    /// Returns the effective timeout in seconds.
    ///
    /// When `test_seconds` is active, minute numbers are treated directly as seconds
    /// (e.g. 5 minutes -> 5 seconds) to allow rapid manual and automated testing.
    pub const fn as_seconds(&self, test_seconds: bool) -> u32 {
        if test_seconds {
            self.as_minutes()
        } else {
            self.as_minutes() * 60
        }
    }

    /// Parses minutes into an `IdleTimeout`, defaulting to `ThirtyMinutes` if unmatched.
    pub fn from_minutes(minutes: u32) -> Self {
        match minutes {
            5 => Self::FiveMinutes,
            15 => Self::FifteenMinutes,
            30 => Self::ThirtyMinutes,
            60 => Self::OneHour,
            120 => Self::TwoHours,
            _ => Self::ThirtyMinutes,
        }
    }

    /// Menu display label for context menus and settings UI.
    pub fn label(&self, test_seconds: bool) -> &'static str {
        if test_seconds {
            match self {
                Self::FiveMinutes => "5 seconds",
                Self::FifteenMinutes => "15 seconds",
                Self::ThirtyMinutes => "30 seconds",
                Self::OneHour => "60 seconds",
                Self::TwoHours => "120 seconds",
            }
        } else {
            match self {
                Self::FiveMinutes => "5 minutes",
                Self::FifteenMinutes => "15 minutes",
                Self::ThirtyMinutes => "30 minutes",
                Self::OneHour => "1 hour",
                Self::TwoHours => "2 hours",
            }
        }
    }
}

/// Explicit states of the IdleOff monitoring lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Idle monitoring is paused by user preference.
    Paused,
    /// Actively polling user idle time in the background.
    Monitoring,
    /// Warning popup is displayed with a live countdown.
    Warning { remaining_seconds: u32 },
    /// Shutdown sequence has been initiated.
    ShuttingDown,
}

/// Action to be performed by the Win32 window layer in response to a state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// No visual or stateful change required.
    None,
    /// Display the warning popup window with the given remaining seconds.
    ShowWarning { remaining_seconds: u32 },
    /// Update the countdown label on the active warning popup.
    UpdateWarning { remaining_seconds: u32 },
    /// Close and hide the warning popup window.
    CloseWarning,
    /// Initiate the system shutdown.
    ExecuteShutdown,
}

/// Pure state machine evaluation step.
///
/// Evaluates the current state against observed idle seconds and produces the next
/// state and corresponding action.
///
/// # Edge Case: 5-Minute Timeout with 3-Minute Warning
/// When the 5-minute timeout is chosen (300 seconds), the warning popup threshold
/// is `300 - 180 = 120 seconds` (2 minutes of idle time). The user receives the
/// complete 3-minute countdown popup starting at minute 2, exactly reaching 0 at minute 5.
pub fn evaluate_tick(
    current_state: AppState,
    idle_seconds: u32,
    timeout_seconds: u32,
    show_popup: bool,
    warning_lead_seconds: u32,
) -> (AppState, Action) {
    match current_state {
        AppState::Paused => (AppState::Paused, Action::None),

        AppState::Monitoring => {
            if !show_popup {
                if idle_seconds >= timeout_seconds {
                    (AppState::ShuttingDown, Action::ExecuteShutdown)
                } else {
                    (AppState::Monitoring, Action::None)
                }
            } else {
                let warning_threshold = timeout_seconds.saturating_sub(warning_lead_seconds);

                if idle_seconds >= timeout_seconds {
                    (AppState::ShuttingDown, Action::ExecuteShutdown)
                } else if idle_seconds >= warning_threshold {
                    let remaining = timeout_seconds.saturating_sub(idle_seconds);
                    (
                        AppState::Warning {
                            remaining_seconds: remaining,
                        },
                        Action::ShowWarning {
                            remaining_seconds: remaining,
                        },
                    )
                } else {
                    (AppState::Monitoring, Action::None)
                }
            }
        }

        AppState::Warning { .. } => {
            let warning_threshold = timeout_seconds.saturating_sub(warning_lead_seconds);

            // If idle time dropped below the threshold, the user provided mouse or keyboard input!
            if idle_seconds < warning_threshold {
                (AppState::Monitoring, Action::CloseWarning)
            } else if idle_seconds >= timeout_seconds {
                (AppState::ShuttingDown, Action::ExecuteShutdown)
            } else {
                let remaining = timeout_seconds.saturating_sub(idle_seconds);
                (
                    AppState::Warning {
                        remaining_seconds: remaining,
                    },
                    Action::UpdateWarning {
                        remaining_seconds: remaining,
                    },
                )
            }
        }

        AppState::ShuttingDown => (AppState::ShuttingDown, Action::None),
    }
}

/// Complete application context holding runtime options, configuration, and state.
pub struct App {
    pub state: AppState,
    pub timeout: IdleTimeout,
    pub show_popup: bool,
    pub test_seconds: bool,
    pub dry_run: bool,
}

impl App {
    /// Creates a new App instance from loaded configuration and command-line flags.
    pub fn new(config: &Config, test_seconds: bool, dry_run: bool) -> Self {
        let state = if config.enabled {
            AppState::Monitoring
        } else {
            AppState::Paused
        };

        Self {
            state,
            timeout: IdleTimeout::from_minutes(config.timeout_minutes),
            show_popup: config.show_popup,
            test_seconds,
            dry_run,
        }
    }

    /// Effective timeout in seconds.
    pub fn effective_timeout_seconds(&self) -> u32 {
        self.timeout.as_seconds(self.test_seconds)
    }

    /// Warning lead time in seconds before shutdown occurs.
    /// In test mode with rapid second-based timeouts, warning lead is 3 seconds.
    /// In normal production mode, warning lead is 180 seconds (3 minutes).
    pub fn warning_lead_seconds(&self) -> u32 {
        if self.test_seconds {
            3
        } else {
            180
        }
    }

    /// Ticks the state machine with observed idle seconds and returns the required action.
    pub fn on_tick(&mut self, idle_seconds: u32) -> Action {
        let prev_state = self.state;
        let (next_state, action) = evaluate_tick(
            self.state,
            idle_seconds,
            self.effective_timeout_seconds(),
            self.show_popup,
            self.warning_lead_seconds(),
        );
        self.state = next_state;
        if prev_state != next_state {
            logger::log(&format!(
                "State transition: {:?} -> {:?} (action: {:?}, idle={}s)",
                prev_state, next_state, action, idle_seconds
            ));
        }
        action
    }

    /// Reloads settings from disk configuration.
    pub fn apply_config(&mut self, config: &Config) -> Action {
        let prev_enabled = !matches!(self.state, AppState::Paused);
        if config.enabled != prev_enabled {
            self.state = if config.enabled {
                AppState::Monitoring
            } else {
                AppState::Paused
            };
        }
        self.timeout = IdleTimeout::from_minutes(config.timeout_minutes);
        self.show_popup = config.show_popup;

        logger::log(&format!(
            "Applied new config: enabled={}, timeout={}m, popup={}",
            config.enabled, config.timeout_minutes, config.show_popup
        ));

        if !config.enabled && matches!(self.state, AppState::Warning { .. }) {
            self.state = AppState::Paused;
            Action::CloseWarning
        } else {
            Action::None
        }
    }

    /// Invoked when the user clicks the "Cancel" button or closes the countdown popup.
    /// Resets state back to `Monitoring` and dismisses the window.
    pub fn on_cancel_warning(&mut self) -> Action {
        if matches!(self.state, AppState::Warning { .. }) {
            logger::log("Warning cancelled by user button/close");
            self.state = AppState::Monitoring;
            Action::CloseWarning
        } else {
            Action::None
        }
    }

    /// Toggles the monitoring state between `Monitoring` and `Paused`.
    #[allow(dead_code)]
    pub fn toggle_enabled(&mut self) -> Action {
        match self.state {
            AppState::Paused => {
                logger::log("User resumed monitoring");
                self.state = AppState::Monitoring;
                Action::None
            }
            AppState::Monitoring => {
                logger::log("User paused monitoring");
                self.state = AppState::Paused;
                Action::None
            }
            AppState::Warning { .. } => {
                logger::log("User paused monitoring during warning");
                self.state = AppState::Paused;
                Action::CloseWarning
            }
            AppState::ShuttingDown => Action::None,
        }
    }

    /// Sets a new idle timeout interval, closing any active warning popup.
    #[allow(dead_code)]
    pub fn set_timeout(&mut self, timeout: IdleTimeout) -> Action {
        logger::log(&format!(
            "User changed timeout to {}m",
            timeout.as_minutes()
        ));
        self.timeout = timeout;
        if matches!(self.state, AppState::Warning { .. }) {
            self.state = AppState::Monitoring;
            Action::CloseWarning
        } else {
            Action::None
        }
    }

    /// Toggles whether the warning popup appears before shutdown.
    #[allow(dead_code)]
    pub fn toggle_popup(&mut self) -> Action {
        self.show_popup = !self.show_popup;
        logger::log(&format!("User toggled popup to {}", self.show_popup));
        if !self.show_popup && matches!(self.state, AppState::Warning { .. }) {
            self.state = AppState::Monitoring;
            Action::CloseWarning
        } else {
            Action::None
        }
    }

    /// Resets the idle cycle on system events such as sleep resume or desktop session unlock.
    pub fn on_system_resume_or_unlock(&mut self) -> Action {
        logger::log("System resume/unlock detected; resetting idle cycle");
        if matches!(self.state, AppState::Warning { .. }) {
            self.state = AppState::Monitoring;
            Action::CloseWarning
        } else {
            Action::None
        }
    }

    /// Synchronizes current settings to a [`Config`] struct for disk persistence.
    #[allow(dead_code)]
    pub fn to_config(&self) -> Config {
        Config {
            enabled: !matches!(self.state, AppState::Paused),
            timeout_minutes: self.timeout.as_minutes(),
            show_popup: self.show_popup,
        }
    }

    /// Tooltip text for the Windows taskbar notification area.
    pub fn tooltip_text(&self) -> String {
        if matches!(self.state, AppState::Paused) {
            "IdleOff - Paused".to_string()
        } else {
            let label = if self.test_seconds {
                match self.timeout {
                    IdleTimeout::FiveMinutes => "5 sec",
                    IdleTimeout::FifteenMinutes => "15 sec",
                    IdleTimeout::ThirtyMinutes => "30 sec",
                    IdleTimeout::OneHour => "60 sec",
                    IdleTimeout::TwoHours => "120 sec",
                }
            } else {
                match self.timeout {
                    IdleTimeout::FiveMinutes => "5 min",
                    IdleTimeout::FifteenMinutes => "15 min",
                    IdleTimeout::ThirtyMinutes => "30 min",
                    IdleTimeout::OneHour => "1 hour",
                    IdleTimeout::TwoHours => "2 hours",
                }
            };
            format!("IdleOff - Shutdown after {label} idle")
        }
    }

    /// Formatted status string for the top line of the tray context menu.
    #[allow(dead_code)]
    pub fn status_menu_text(&self) -> String {
        if matches!(self.state, AppState::Paused) {
            "Status: Paused".to_string()
        } else {
            format!(
                "Status: Monitoring | Shutdown after {}",
                self.timeout.label(self.test_seconds)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_id_mapping() {
        // ID 0 (menu dismissed) must map strictly to None, never Exit!
        assert_eq!(map_menu_id(0), MenuAction::None);
        // Arbitrary unknown IDs must map strictly to None, never Exit!
        assert_eq!(map_menu_id(9999), MenuAction::None);

        assert_eq!(map_menu_id(IDM_OPEN_SETTINGS), MenuAction::OpenSettings);
        assert_eq!(map_menu_id(IDM_EXIT), MenuAction::Exit);
    }

    #[test]
    fn test_timeout_seconds_conversion() {
        assert_eq!(IdleTimeout::FiveMinutes.as_seconds(false), 300);
        assert_eq!(IdleTimeout::FifteenMinutes.as_seconds(false), 900);
        assert_eq!(IdleTimeout::ThirtyMinutes.as_seconds(false), 1800);
        assert_eq!(IdleTimeout::OneHour.as_seconds(false), 3600);
        assert_eq!(IdleTimeout::TwoHours.as_seconds(false), 7200);

        // Test mode: treated as seconds directly
        assert_eq!(IdleTimeout::FiveMinutes.as_seconds(true), 5);
        assert_eq!(IdleTimeout::FifteenMinutes.as_seconds(true), 15);
    }

    #[test]
    fn test_all_timeout_threshold_rules() {
        let lead_secs = 180; // 3 min

        // 5 min (300s): threshold is 300 - 180 = 120s (2 min idle)
        let (s, a) = evaluate_tick(AppState::Monitoring, 119, 300, true, lead_secs);
        assert_eq!(s, AppState::Monitoring);
        assert_eq!(a, Action::None);

        let (s, a) = evaluate_tick(AppState::Monitoring, 120, 300, true, lead_secs);
        assert_eq!(
            s,
            AppState::Warning {
                remaining_seconds: 180
            }
        );
        assert_eq!(
            a,
            Action::ShowWarning {
                remaining_seconds: 180
            }
        );

        // 15 min (900s): threshold is 900 - 180 = 720s (12 min idle)
        let (s, a) = evaluate_tick(AppState::Monitoring, 720, 900, true, lead_secs);
        assert_eq!(
            s,
            AppState::Warning {
                remaining_seconds: 180
            }
        );
        assert_eq!(
            a,
            Action::ShowWarning {
                remaining_seconds: 180
            }
        );

        // 30 min (1800s): threshold is 1800 - 180 = 1620s (27 min idle)
        let (s, a) = evaluate_tick(AppState::Monitoring, 1620, 1800, true, lead_secs);
        assert_eq!(
            s,
            AppState::Warning {
                remaining_seconds: 180
            }
        );
        assert_eq!(
            a,
            Action::ShowWarning {
                remaining_seconds: 180
            }
        );

        // 1 hour (3600s): threshold is 3600 - 180 = 3420s (57 min idle)
        let (s, a) = evaluate_tick(AppState::Monitoring, 3420, 3600, true, lead_secs);
        assert_eq!(
            s,
            AppState::Warning {
                remaining_seconds: 180
            }
        );
        assert_eq!(
            a,
            Action::ShowWarning {
                remaining_seconds: 180
            }
        );

        // 2 hours (7200s): threshold is 7200 - 180 = 7020s (117 min idle)
        let (s, a) = evaluate_tick(AppState::Monitoring, 7020, 7200, true, lead_secs);
        assert_eq!(
            s,
            AppState::Warning {
                remaining_seconds: 180
            }
        );
        assert_eq!(
            a,
            Action::ShowWarning {
                remaining_seconds: 180
            }
        );
    }

    #[test]
    fn test_warning_dismissed_when_user_moves_mouse() {
        let timeout_secs = 1800; // 30 min
        let lead_secs = 180;

        let state = AppState::Warning {
            remaining_seconds: 100,
        };
        // User moves mouse: idle time drops to 0 seconds
        let (next_state, action) = evaluate_tick(state, 0, timeout_secs, true, lead_secs);
        assert_eq!(next_state, AppState::Monitoring);
        assert_eq!(action, Action::CloseWarning);
    }

    #[test]
    fn test_popup_disabled_immediate_shutdown() {
        let timeout_secs = 1800;
        let lead_secs = 180;

        let (state, act) =
            evaluate_tick(AppState::Monitoring, 1700, timeout_secs, false, lead_secs);
        assert_eq!(state, AppState::Monitoring);
        assert_eq!(act, Action::None);

        let (state, act) =
            evaluate_tick(AppState::Monitoring, 1800, timeout_secs, false, lead_secs);
        assert_eq!(state, AppState::ShuttingDown);
        assert_eq!(act, Action::ExecuteShutdown);
    }

    #[test]
    fn test_cancel_button_action() {
        let mut app = App::new(&Config::default(), false, false);
        app.state = AppState::Warning {
            remaining_seconds: 120,
        };
        let action = app.on_cancel_warning();
        assert_eq!(app.state, AppState::Monitoring);
        assert_eq!(action, Action::CloseWarning);
    }
}
