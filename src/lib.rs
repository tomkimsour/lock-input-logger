use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use evdev::{EventSummary, InputEvent, KeyCode};

/// Coarse kind of input activity, deliberately too coarse to reveal which key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Typing,
    Repeat,
    Media,
    Button,
    Pointer,
}

const MEDIA_KEYS: &[KeyCode] = &[
    KeyCode::KEY_MUTE,
    KeyCode::KEY_VOLUMEDOWN,
    KeyCode::KEY_VOLUMEUP,
    KeyCode::KEY_NEXTSONG,
    KeyCode::KEY_PLAYPAUSE,
    KeyCode::KEY_PREVIOUSSONG,
    KeyCode::KEY_STOPCD,
    KeyCode::KEY_PLAYCD,
    KeyCode::KEY_PAUSECD,
    KeyCode::KEY_PLAY,
    KeyCode::KEY_PAUSE,
    KeyCode::KEY_FASTFORWARD,
    KeyCode::KEY_REWIND,
];

fn is_button(code: KeyCode) -> bool {
    (KeyCode::BTN_0.0..KeyCode::KEY_OK.0).contains(&code.0)
        || (KeyCode::BTN_TRIGGER_HAPPY1.0..=KeyCode::BTN_TRIGGER_HAPPY40.0).contains(&code.0)
}

/// Maps a raw evdev event to a coarse activity category, or `None` for events not worth logging.
pub fn classify(event: &InputEvent) -> Option<Category> {
    match event.destructure() {
        EventSummary::Key(_, _, 2) => Some(Category::Repeat),
        EventSummary::Key(_, code, 1) if MEDIA_KEYS.contains(&code) => Some(Category::Media),
        EventSummary::Key(_, code, 1) if is_button(code) => Some(Category::Button),
        EventSummary::Key(_, _, 1) => Some(Category::Typing),
        EventSummary::RelativeAxis(..) | EventSummary::AbsoluteAxis(..) => Some(Category::Pointer),
        _ => None,
    }
}

/// Returns the logind session id belonging to `user` on `seat0`, given `(sid, user, seat)` tuples.
pub fn pick_session(sessions: &[(String, String, String)], user: &str) -> Option<String> {
    sessions
        .iter()
        .find(|(_, u, seat)| u == user && seat == "seat0")
        .map(|(sid, _, _)| sid.clone())
}

/// Returns the configured target user, treating a missing or blank value as unset.
pub fn target_user(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

struct Burst {
    first_label: String,
    last_label: String,
    last_seen: Instant,
    counts: [u32; 5],
}

impl Burst {
    fn summary(&self, device: &str) -> String {
        let [typing, repeat, media, button, pointer] = self.counts;
        format!(
            "{}-{} '{device}' typing={typing} repeat={repeat} media={media} button={button} pointer={pointer}",
            self.first_label, self.last_label
        )
    }
}

/// Groups activity per device into bursts and renders one summary line per finished burst.
pub struct BurstAggregator {
    gap: Duration,
    bursts: BTreeMap<String, Burst>,
}

impl BurstAggregator {
    /// Creates an aggregator that closes a device's burst after `gap` without activity.
    pub fn new(gap: Duration) -> Self {
        Self {
            gap,
            bursts: BTreeMap::new(),
        }
    }

    /// Records one activity event of `category` from `device` at `now`, displayed as `label`.
    pub fn record(&mut self, device: &str, category: Category, now: Instant, label: &str) {
        let burst = self
            .bursts
            .entry(device.to_owned())
            .or_insert_with(|| Burst {
                first_label: label.to_owned(),
                last_label: label.to_owned(),
                last_seen: now,
                counts: [0; 5],
            });
        burst.last_label = label.to_owned();
        burst.last_seen = now;
        burst.counts[category as usize] += 1;
    }

    /// Returns summary lines for bursts quiet for at least the gap, or all bursts when `force` is set.
    pub fn flush(&mut self, now: Instant, force: bool) -> Vec<String> {
        let gap = self.gap;
        let done: Vec<String> = self
            .bursts
            .iter()
            .filter(|(_, b)| force || now.saturating_duration_since(b.last_seen) >= gap)
            .map(|(device, _)| device.clone())
            .collect();
        done.iter()
            .filter_map(|device| self.bursts.remove(device).map(|b| b.summary(device)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::{EventType, KeyCode, RelativeAxisCode};

    fn key(code: KeyCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code.0, value)
    }

    fn sessions() -> Vec<(String, String, String)> {
        vec![
            ("4".into(), "thomasung".into(), "".into()),
            ("3".into(), "thomasung".into(), "seat0".into()),
            ("7".into(), "greeter".into(), "seat0".into()),
        ]
    }

    #[test]
    fn should_return_none_when_user_is_empty() {
        let raw = Some(String::new());

        let user = target_user(raw);

        assert_eq!(user, None);
    }

    #[test]
    fn should_return_none_when_user_is_whitespace() {
        let raw = Some("  ".to_owned());

        let user = target_user(raw);

        assert_eq!(user, None);
    }

    #[test]
    fn should_trim_surrounding_whitespace_from_user() {
        let raw = Some(" thomasung ".to_owned());

        let user = target_user(raw);

        assert_eq!(user.as_deref(), Some("thomasung"));
    }

    #[test]
    fn should_classify_letter_press_as_typing() {
        let event = key(KeyCode::KEY_A, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Typing));
    }

    #[test]
    fn should_classify_autorepeat_as_repeat() {
        let event = key(KeyCode::KEY_A, 2);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Repeat));
    }

    #[test]
    fn should_ignore_key_release() {
        let event = key(KeyCode::KEY_A, 0);

        let category = classify(&event);

        assert_eq!(category, None);
    }

    #[test]
    fn should_classify_playpause_as_media() {
        let event = key(KeyCode::KEY_PLAYPAUSE, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Media));
    }

    #[test]
    fn should_classify_pausecd_as_media() {
        let event = key(KeyCode::KEY_PAUSECD, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Media));
    }

    #[test]
    fn should_classify_volume_up_as_media() {
        let event = key(KeyCode::KEY_VOLUMEUP, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Media));
    }

    #[test]
    fn should_classify_left_click_as_button() {
        let event = key(KeyCode::BTN_LEFT, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Button));
    }

    #[test]
    fn should_classify_touchpad_touch_as_button() {
        let event = key(KeyCode::BTN_TOUCH, 1);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Button));
    }

    #[test]
    fn should_classify_relative_motion_as_pointer() {
        let event = InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, 3);

        let category = classify(&event);

        assert_eq!(category, Some(Category::Pointer));
    }

    #[test]
    fn should_ignore_sync_events() {
        let event = InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0);

        let category = classify(&event);

        assert_eq!(category, None);
    }

    #[test]
    fn should_pick_seat0_session_of_user() {
        let all = sessions();

        let picked = pick_session(&all, "thomasung");

        assert_eq!(picked.as_deref(), Some("3"));
    }

    #[test]
    fn should_return_none_when_user_has_no_seat_session() {
        let all = sessions();

        let picked = pick_session(&all, "nobody");

        assert_eq!(picked, None);
    }

    #[test]
    fn should_not_flush_burst_still_in_progress() {
        let start = Instant::now();
        let mut agg = BurstAggregator::new(Duration::from_secs(3));
        agg.record("Dell Keyboard", Category::Typing, start, "12:00:00");

        let lines = agg.flush(start + Duration::from_secs(1), false);

        assert!(lines.is_empty());
    }

    #[test]
    fn should_flush_burst_after_gap() {
        let start = Instant::now();
        let mut agg = BurstAggregator::new(Duration::from_secs(3));
        agg.record("Dell Keyboard", Category::Typing, start, "12:00:00");

        let lines = agg.flush(start + Duration::from_secs(4), false);

        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn should_flush_everything_when_forced() {
        let start = Instant::now();
        let mut agg = BurstAggregator::new(Duration::from_secs(3));
        agg.record("Dell Keyboard", Category::Typing, start, "12:00:00");
        agg.record("MOMENTUM 4 (AVRCP)", Category::Media, start, "12:00:00");

        let lines = agg.flush(start, true);

        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn should_summarize_device_time_range_and_counts() {
        let start = Instant::now();
        let mut agg = BurstAggregator::new(Duration::from_secs(3));
        agg.record("Dell Keyboard", Category::Typing, start, "12:00:00");
        agg.record(
            "Dell Keyboard",
            Category::Typing,
            start + Duration::from_secs(1),
            "12:00:01",
        );
        agg.record(
            "Dell Keyboard",
            Category::Repeat,
            start + Duration::from_secs(2),
            "12:00:02",
        );

        let lines = agg.flush(start + Duration::from_secs(10), false);

        assert_eq!(
            lines,
            vec!["12:00:00-12:00:02 'Dell Keyboard' typing=2 repeat=1 media=0 button=0 pointer=0"]
        );
    }

    #[test]
    fn should_start_new_burst_after_flush() {
        let start = Instant::now();
        let mut agg = BurstAggregator::new(Duration::from_secs(3));
        agg.record("Dell Keyboard", Category::Typing, start, "12:00:00");
        agg.flush(start + Duration::from_secs(10), false);
        agg.record(
            "Dell Keyboard",
            Category::Media,
            start + Duration::from_secs(20),
            "12:00:20",
        );

        let lines = agg.flush(start + Duration::from_secs(30), false);

        assert_eq!(
            lines,
            vec!["12:00:20-12:00:20 'Dell Keyboard' typing=0 repeat=0 media=1 button=0 pointer=0"]
        );
    }
}
