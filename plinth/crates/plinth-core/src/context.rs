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

/// Parse "HH:MM" into minutes since midnight.
pub fn parse_hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn fmt_hhmm(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn valid_exe(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && a.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b' ' | b'(' | b')'))
}

/// Rules are stored and exchanged as small JSON objects:
/// `{"name":"Coding","when":{"kind":"app","value":"code.exe"},"profile":"Work"}`; kinds: app, fullscreen, battery, time (from, to as HH:MM).
/// Anything malformed is rejected, never guessed.
pub fn rule_from_json(v: &serde_json::Value) -> Result<Rule, String> {
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(|c| c.is_control()) {
        return Err("The rule needs a short name.".into());
    }
    let profile = v.get("profile").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    if profile.is_empty() || profile.chars().count() > 40 || profile.chars().any(|c| c.is_control()) {
        return Err("Choose a profile for the rule.".into());
    }
    let w = v.get("when").ok_or("The rule needs a condition.")?;
    let when = match w.get("kind").and_then(|x| x.as_str()).unwrap_or("") {
        "app" => {
            let a = w.get("value").and_then(|x| x.as_str()).unwrap_or("").trim();
            if !valid_exe(a) {
                return Err("Type the program's file name, for example code.exe.".into());
            }
            Condition::AppForeground(a.to_string())
        }
        "fullscreen" => Condition::Fullscreen,
        "battery" => Condition::OnBattery,
        "time" => {
            let (f, t) = (w.get("from").and_then(|x| x.as_str()).and_then(parse_hhmm), w.get("to").and_then(|x| x.as_str()).and_then(parse_hhmm));
            match (f, t) {
                (Some(from), Some(to)) if from != to => Condition::TimeBetween { from, to },
                _ => return Err("Use two different times like 22:00 and 06:00.".into()),
            }
        }
        _ => return Err("Unknown condition.".into()),
    };
    Ok(Rule { name, when, scene: profile })
}

pub fn rule_to_json(r: &Rule) -> serde_json::Value {
    let when = match &r.when {
        Condition::AppForeground(a) => serde_json::json!({ "kind": "app", "value": a }),
        Condition::Fullscreen => serde_json::json!({ "kind": "fullscreen" }),
        Condition::OnBattery => serde_json::json!({ "kind": "battery" }),
        Condition::TimeBetween { from, to } => serde_json::json!({ "kind": "time", "from": fmt_hhmm(*from), "to": fmt_hhmm(*to) }),
        Condition::All(_) | Condition::Always => serde_json::json!({ "kind": "always" }),
    };
    serde_json::json!({ "name": r.name, "when": when, "profile": r.scene })
}

/// Plain-language description of a condition for the hub.
pub fn describe(c: &Condition) -> String {
    match c {
        Condition::AppForeground(a) => format!("when I am using {a}"),
        Condition::Fullscreen => "when a fullscreen app or game is open".into(),
        Condition::OnBattery => "when on battery".into(),
        Condition::TimeBetween { from, to } => format!("between {} and {}", fmt_hhmm(*from), fmt_hhmm(*to)),
        Condition::All(_) => "when several things are true".into(),
        Condition::Always => "otherwise".into(),
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

    #[test]
    fn rules_round_trip_through_json_and_reject_junk() {
        for r in rules().into_iter().filter(|r| !matches!(r.when, Condition::Always)) {
            let r2 = rule_from_json(&rule_to_json(&r)).unwrap();
            assert_eq!((r2.name.as_str(), &r2.when, r2.scene.as_str()), (r.name.as_str(), &r.when, r.scene.as_str()));
        }
        let bad = |j: serde_json::Value| rule_from_json(&j).is_err();
        assert!(bad(serde_json::json!({})));
        assert!(bad(serde_json::json!({"name":"","when":{"kind":"battery"},"profile":"P"})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"battery"},"profile":""})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"app","value":"..\\evil.exe"},"profile":"P"})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"app","value":""},"profile":"P"})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"time","from":"25:00","to":"06:00"},"profile":"P"})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"time","from":"07:00","to":"07:00"},"profile":"P"})));
        assert!(bad(serde_json::json!({"name":"x","when":{"kind":"magic"},"profile":"P"})));
        assert_eq!(parse_hhmm("22:30"), Some(1350));
        assert_eq!(parse_hhmm("7:5"), Some(425));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(fmt_hhmm(1350), "22:30");
        assert_eq!(describe(&Condition::AppForeground("code.exe".into())), "when I am using code.exe");
    }
}
