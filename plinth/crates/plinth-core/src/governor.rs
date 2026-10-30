//! Minimal pause governor (MVP M3): pure decision function. OS probing lives outside this crate.

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemState {
    pub fullscreen_app: bool,
    pub game_running: bool,
    pub on_battery: bool,
    pub battery_percent: u8,
    pub remote_session: bool,
    pub display_off: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub pause_on_fullscreen: bool,
    pub pause_on_battery_below: Option<u8>,
    pub pause_in_remote_session: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self { pause_on_fullscreen: true, pause_on_battery_below: Some(20), pause_in_remote_session: true }
    }
}

pub fn should_pause(s: &SystemState, p: &Policy) -> bool {
    s.display_off
        || (p.pause_on_fullscreen && (s.fullscreen_app || s.game_running))
        || (p.pause_in_remote_session && s.remote_session)
        || matches!(p.pause_on_battery_below, Some(t) if s.on_battery && s.battery_percent < t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_desktop_runs() {
        assert!(!should_pause(&SystemState::default(), &Policy::default()));
    }

    #[test]
    fn pauses_for_each_trigger() {
        let p = Policy::default();
        assert!(should_pause(&SystemState { fullscreen_app: true, ..Default::default() }, &p));
        assert!(should_pause(&SystemState { game_running: true, ..Default::default() }, &p));
        assert!(should_pause(&SystemState { remote_session: true, ..Default::default() }, &p));
        assert!(should_pause(&SystemState { display_off: true, ..Default::default() }, &p));
        assert!(should_pause(&SystemState { on_battery: true, battery_percent: 10, ..Default::default() }, &p));
        assert!(!should_pause(&SystemState { on_battery: true, battery_percent: 80, ..Default::default() }, &p));
    }

    #[test]
    fn policy_can_disable_triggers() {
        let p = Policy { pause_on_fullscreen: false, ..Policy::default() };
        assert!(!should_pause(&SystemState { fullscreen_app: true, ..Default::default() }, &p));
        // display-off always pauses
        assert!(should_pause(&SystemState { display_off: true, ..Default::default() }, &p));
    }
}
