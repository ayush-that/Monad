//! Battery state management.

use battery::{Manager, State};
use dioxus::prelude::*;

/// Battery state information.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BatteryInfo {
    /// Battery percentage (0-100).
    pub percentage: f32,
    /// Whether the battery is charging.
    pub is_charging: bool,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            percentage: 100.0,
            is_charging: false,
        }
    }
}

/// Battery state for the application.
#[derive(Clone)]
pub struct BatteryState {
    /// Current battery info.
    pub info: Signal<BatteryInfo>,
}

impl BatteryState {
    /// Create a new battery state.
    pub fn new() -> Self {
        let initial_info = get_battery_info().unwrap_or_default();
        Self {
            info: Signal::new(initial_info),
        }
    }

    /// Update the battery info from the system.
    pub fn refresh(&mut self) {
        if let Some(info) = get_battery_info() {
            self.info.set(info);
        }
    }
}

impl Default for BatteryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current battery information from the system.
pub fn get_battery_info() -> Option<BatteryInfo> {
    let manager = Manager::new().ok()?;
    let mut batteries = manager.batteries().ok()?;
    let battery = batteries.next()?.ok()?;

    let percentage = battery.state_of_charge().value * 100.0;
    let is_charging = matches!(battery.state(), State::Charging | State::Full);

    Some(BatteryInfo {
        percentage,
        is_charging,
    })
}
