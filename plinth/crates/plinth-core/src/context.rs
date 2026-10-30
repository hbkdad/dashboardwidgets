//! Moods: context-aware scene switching that stays predictable.
//! Research note (design/ux-flows.md, research/consumer-demand.md): static rules disappoint people, so the engine
//! (1) never fights a manual choice made in the same context, (2) has a minimum dwell time to avoid flapping, and
//! (3) the `Learner` only SUGGESTS a new rule after the same manual switch repeats; it never creates rules silently.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Foreground executable name, compared case-insensitively (for example "code.exe").
    AppForeground(String),
    Fullscreen,
    OnBattery,
    /// Minutes since midnight; `from > to` wraps past midnight.
    TimeBetween { from: u16, to: u16 },
    All(Vec<Condition>),
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub when: Condition,
    pub scene: String,
}

#[derive(Debug, Clone, Default)]
pub struct Context {
    pub foreground_app: String,
    pub fullscreen: bool,
    pub on_battery: bool,
    pub minute_of_day: u16,
    pub now_secs: u64,
}

impl Context {
    /// Identity of the situation, excluding time: manual overrides last until this changes.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.foreground_app.to_ascii_lowercase(), self.fullscreen, self.on_battery)
    }
}

impl Condition {
    pub fn matches(&self, c: &Context) -> bool {
        match self {
            Condition::AppForeground(a) => !a.is_empty() && c.foreground_app.eq_ignore_ascii_case(a),
            Condition::Fullscreen => c.fullscreen,
            Condition::OnBattery => c.on_battery,
            Condition::TimeBetween { from, to } => {
                let m = c.minute_of_day;
                if from <= to {
                    m >= *from && m < *to
                } else {
                    m >= *from || m < *to
                }
            }
            Condition::All(v) => !v.is_empty() && v.iter().all(|x| x.matches(c)),
            Condition::Always => true,
        }
    }
}

pub struct Engine {
    rules: Vec<Rule>,
    current: Option<String>,
    last_switch_secs: Option<u64>,
    override_key: Option<String>,
    min_dwell_secs: u64,
}

impl Engine {
    pub fn new(rules: Vec<Rule>, min_dwell_secs: u64) -> Self {
        Engine { rules, current: None, last_switch_secs: None, override_key: None, min_dwell_secs }
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// The user picked a scene by hand. Automation stands down until the context changes.
    pub fn manual_switch(&mut self, scene: &str, ctx: &Context) {
        self.current = Some(scene.to_string());
        self.last_switch_secs = Some(ctx.now_secs);
        self.override_key = Some(ctx.key());
    }

    /// Returns the scene to switch to, if a change is warranted. The first matching rule wins (order = priority).
    pub fn evaluate(&mut self, ctx: &Context) -> Option<String> {
        if let Some(k) = &self.override_key {
            if *k == ctx.key() {
                return None;
            }
            self.override_key = None;
        }
        let target = self.rules.iter().find(|r| r.when.matches(ctx))?.scene.clone();
        if self.current.as_deref() == Some(target.as_str()) {
            return None;
        }
        if let Some(t) = self.last_switch_secs {
            if ctx.now_secs.saturating_sub(t) < self.min_dwell_secs {
                return None;
            }
        }
        self.current = Some(target.clone());
        self.last_switch_secs = Some(ctx.now_secs);
        Some(target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub when: Condition,
    pub scene: String,
}

/// Counts identical manual switches per situation and proposes a rule after `threshold` of them.
pub struct Learner {
    counts: HashMap<(String, String), u32>,
    dismissed: HashSet<(String, String)>,
    suggested: HashSet<(String, String)>,
    threshold: u32,
}

impl Learner {
    pub fn new(threshold: u32) -> Self {
        Learner { counts: HashMap::new(), dismissed: HashSet::new(), suggested: HashSet::new(), threshold: threshold.max(2) }
    }

    pub fn record_manual(&mut self, ctx: &Context, scene: &str) -> Option<Suggestion> {
        let cond = if ctx.fullscreen {
            Condition::Fullscreen
        } else if !ctx.foreground_app.is_empty() {
            Condition::AppForeground(ctx.foreground_app.to_ascii_lowercase())
        } else if ctx.on_battery {
            Condition::OnBattery
        } else {
            return None; // nothing specific enough to automate
        };
        let key = (ctx.key(), scene.to_string());
        if self.dismissed.contains(&key) || self.suggested.contains(&key) {
            return None;
        }
        let n = self.counts.entry(key.clone()).or_insert(0);
        *n += 1;
        if *n >= self.threshold {
            self.suggested.insert(key);
            return Some(Suggestion { when: cond, scene: scene.to_string() });
        }
        None
    }

    /// "Not now / never": do not suggest this exact situation and scene again.
    pub fn dismiss(&mut self, ctx: &Context, scene: &str) {
        self.dismissed.insert((ctx.key(), scene.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(app: &str, fs: bool, batt: bool, minute: u16, now: u64) -> Context {
        Context { foreground_app: app.into(), fullscreen: fs, on_battery: batt, minute_of_day: minute, now_secs: now }
    }
    fn rules() -> Vec<Rule> {
        vec![
            Rule { name: "Gaming".into(), when: Condition::Fullscreen, scene: "graphite".into() },
            Rule { name: "Coding".into(), when: Condition::AppForeground("Code.exe".into()), scene: "focus".into() },
            Rule { name: "Night".into(), when: Condition::TimeBetween { from: 22 * 60, to: 6 * 60 }, scene: "dusk".into() },
            Rule { name: "Default".into(), when: Condition::Always, scene: "paper".into() },
        ]
    }

    #[test]
    fn conditions_match_including_midnight_wrap_and_case() {
        let t = Condition::TimeBetween { from: 22 * 60, to: 6 * 60 };
        assert!(t.matches(&ctx("", false, false, 23 * 60, 0)));
        assert!(t.matches(&ctx("", false, false, 2 * 60, 0)));
        assert!(!t.matches(&ctx("", false, false, 12 * 60, 0)));
        assert!(!t.matches(&ctx("", false, false, 6 * 60, 0)), "end is exclusive");
        assert!(Condition::AppForeground("Code.exe".into()).matches(&ctx("code.EXE", false, false, 0, 0)));
        assert!(!Condition::AppForeground("".into()).matches(&ctx("", false, false, 0, 0)), "empty app never matches");
        assert!(!Condition::All(vec![]).matches(&ctx("", false, false, 0, 0)));
        assert!(Condition::All(vec![Condition::OnBattery, Condition::Fullscreen]).matches(&ctx("", true, true, 0, 0)));
    }

    #[test]
    fn first_matching_rule_wins() {
        let mut e = Engine::new(rules(), 60);
        assert_eq!(e.evaluate(&ctx("code.exe", true, false, 600, 1000)).as_deref(), Some("graphite"), "fullscreen outranks coding");
    }

    #[test]
    fn dwell_time_prevents_flapping() {
        let mut e = Engine::new(rules(), 60);
        assert_eq!(e.evaluate(&ctx("code.exe", false, false, 600, 1000)).as_deref(), Some("focus"));
        assert_eq!(e.evaluate(&ctx("", false, false, 600, 1010)), None, "10 s later: too soon to switch back");
        assert_eq!(e.evaluate(&ctx("", false, false, 600, 1070)).as_deref(), Some("paper"));
        assert_eq!(e.evaluate(&ctx("", false, false, 600, 2000)), None, "already on target");
    }

    #[test]
    fn manual_choice_is_respected_until_context_changes() {
        let mut e = Engine::new(rules(), 0);
        let c = ctx("chrome.exe", false, false, 600, 1000);
        e.manual_switch("graphite", &c);
        assert_eq!(e.evaluate(&ctx("chrome.exe", false, false, 601, 1100)), None, "same situation: do not fight the user");
        assert_eq!(e.current(), Some("graphite"));
        assert_eq!(e.evaluate(&ctx("code.exe", false, false, 602, 1200)).as_deref(), Some("focus"), "context changed: automation resumes");
    }

    #[test]
    fn no_rule_means_no_change() {
        let mut e = Engine::new(vec![Rule { name: "x".into(), when: Condition::Fullscreen, scene: "g".into() }], 0);
        assert_eq!(e.evaluate(&ctx("", false, false, 0, 0)), None);
    }

    #[test]
    fn learner_suggests_after_repeats_once_and_respects_dismissal() {
        let mut l = Learner::new(3);
        let c = ctx("Blender.exe", false, false, 0, 0);
        assert_eq!(l.record_manual(&c, "graphite"), None);
        assert_eq!(l.record_manual(&c, "graphite"), None);
        let s = l.record_manual(&c, "graphite").unwrap();
        assert_eq!(s, Suggestion { when: Condition::AppForeground("blender.exe".into()), scene: "graphite".into() });
        assert_eq!(l.record_manual(&c, "graphite"), None, "only suggested once");
        // different scene counts separately; dismissal sticks
        let mut l2 = Learner::new(2);
        l2.dismiss(&c, "dusk");
        assert_eq!(l2.record_manual(&c, "dusk"), None);
        assert_eq!(l2.record_manual(&c, "dusk"), None);
        // nothing specific to automate
        assert_eq!(Learner::new(2).record_manual(&ctx("", false, false, 0, 0), "x"), None);
        assert_eq!(l.record_manual(&ctx("", true, false, 0, 0), "graphite"), None, "first fullscreen count");
    }
}
