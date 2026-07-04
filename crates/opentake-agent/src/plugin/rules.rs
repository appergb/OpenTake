//! Plugin `workflow.rules` validation (`agent-SPEC.md` §6.6.2). The plugin's
//! `dont` list is an extra rule layer on top of the built-in rules; both apply
//! (order: built-in → plugin). Machine-decidable rules are checked structurally
//! (e.g. "no more than N consecutive uncovered MainCamera segments"); the rest
//! degrade to soft reminders surfaced verbatim for the LLM to self-check.

use opentake_domain::{Timeline, TrackRole, TrackRoleAssignment};

use crate::plugin::registry::LoadedPlugin;

/// A small set of regex-free structural matchers for common `dont` phrasings.
/// Anything not matched is returned as a soft reminder (verbatim).
pub fn plugin_rules(
    plugin: Option<&LoadedPlugin>,
    roles: &[TrackRoleAssignment],
    timeline: &Timeline,
) -> Vec<String> {
    let Some(plugin) = plugin else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for dont in &plugin.manifest.workflow.rules.dont {
        if let Some(threshold) = parse_consecutive_no_broll(dont) {
            if let Some(run) = max_uncovered_main_camera_run(roles, timeline) {
                if run >= threshold {
                    warnings.push(format!(
                        "工作流规则[plugin:{}]：{} （检测到连续 {} 段主画面无 B-roll 覆盖）",
                        plugin.id(),
                        dont,
                        run
                    ));
                }
            }
        } else {
            // Not machine-decidable -> soft reminder, verbatim, tagged.
            warnings.push(format!("工作流规则[plugin:{}]：{}", plugin.id(), dont));
        }
    }
    warnings
}

/// Match phrasings like "不要连续 3 段以上无 B-roll 覆盖" and return the
/// threshold (here 3). Returns `None` when the phrase isn't this shape.
fn parse_consecutive_no_broll(s: &str) -> Option<usize> {
    if !(s.contains("连续") && s.contains("B-roll")) {
        return None;
    }
    // Pull the first run of ASCII digits as the threshold.
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<usize>().ok()
}

/// Longest run of MainCamera clips (sorted by start) with no overlapping BRoll
/// clip on any B-roll track. A structural proxy for "uncovered segments".
fn max_uncovered_main_camera_run(
    roles: &[TrackRoleAssignment],
    timeline: &Timeline,
) -> Option<usize> {
    let main_idx = roles
        .iter()
        .find(|a| a.role == TrackRole::MainCamera)
        .map(|a| a.track_index)?;
    if main_idx >= timeline.tracks.len() {
        return None;
    }
    let broll_indices: Vec<usize> = roles
        .iter()
        .filter(|a| a.role == TrackRole::BRoll)
        .map(|a| a.track_index)
        .collect();

    let mut main_clips: Vec<&opentake_domain::Clip> =
        timeline.tracks[main_idx].clips.iter().collect();
    main_clips.sort_by_key(|c| c.start_frame);

    let mut best = 0usize;
    let mut run = 0usize;
    for clip in main_clips {
        let covered = broll_indices.iter().any(|&bi| {
            timeline.tracks[bi].clips.iter().any(|b| {
                // Overlap test on [start, end).
                b.start_frame < clip.end_frame() && clip.start_frame < b.end_frame()
            })
        });
        if covered {
            run = 0;
        } else {
            run += 1;
            best = best.max(run);
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::registry::PluginRegistry;
    use opentake_domain::{Clip, ClipType, Timeline, Track};

    fn plugin_with_dont(donts: &[&str]) -> LoadedPlugin {
        let dont_json: Vec<String> = donts.iter().map(|d| format!("\"{d}\"")).collect();
        let json = format!(
            r#"{{"schema_version":"1.0","id":"wp","name":"WP","workflow":{{"rules":{{"do":[],"dont":[{}]}}}}}}"#,
            dont_json.join(",")
        );
        PluginRegistry::load_from_strings(&json, "", ".").unwrap()
    }

    fn roles(main: usize, broll: usize) -> Vec<TrackRoleAssignment> {
        vec![
            TrackRoleAssignment {
                track_index: main,
                role: TrackRole::MainCamera,
            },
            TrackRoleAssignment {
                track_index: broll,
                role: TrackRole::BRoll,
            },
        ]
    }

    #[test]
    fn no_plugin_no_warnings() {
        let w = plugin_rules(None, &[], &Timeline::new());
        assert!(w.is_empty());
    }

    #[test]
    fn parse_consecutive_threshold() {
        assert_eq!(
            parse_consecutive_no_broll("不要连续 3 段以上无 B-roll 覆盖"),
            Some(3)
        );
        assert_eq!(parse_consecutive_no_broll("不要用花哨转场"), None);
    }

    #[test]
    fn uncovered_main_camera_run_triggers_warning() {
        // 3 main-camera clips, no B-roll coverage -> run = 3 >= threshold 3.
        let mut tl = Timeline::new();
        let mut main = Track::new("v1", ClipType::Video);
        main.clips.push(Clip::new("m1", "a", 0, 30));
        main.clips.push(Clip::new("m2", "a", 30, 30));
        main.clips.push(Clip::new("m3", "a", 60, 30));
        let broll = Track::new("v2", ClipType::Video); // empty
        tl.tracks.push(main);
        tl.tracks.push(broll);

        let plugin = plugin_with_dont(&["不要连续 3 段以上无 B-roll 覆盖"]);
        let w = plugin_rules(Some(&plugin), &roles(0, 1), &tl);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("plugin:wp"), "{}", w[0]);
        assert!(w[0].contains("连续 3 段"), "{}", w[0]);
    }

    #[test]
    fn covered_segments_no_warning() {
        let mut tl = Timeline::new();
        let mut main = Track::new("v1", ClipType::Video);
        main.clips.push(Clip::new("m1", "a", 0, 30));
        main.clips.push(Clip::new("m2", "a", 30, 30));
        main.clips.push(Clip::new("m3", "a", 60, 30));
        let mut broll = Track::new("v2", ClipType::Video);
        // B-roll covering every main clip span.
        broll.clips.push(Clip::new("b1", "x", 0, 90));
        tl.tracks.push(main);
        tl.tracks.push(broll);

        let plugin = plugin_with_dont(&["不要连续 3 段以上无 B-roll 覆盖"]);
        let w = plugin_rules(Some(&plugin), &roles(0, 1), &tl);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn non_decidable_dont_is_soft_reminder() {
        let plugin = plugin_with_dont(&["不要使用过于花哨的转场"]);
        let w = plugin_rules(Some(&plugin), &[], &Timeline::new());
        assert_eq!(w.len(), 1);
        assert_eq!(w[0], "工作流规则[plugin:wp]：不要使用过于花哨的转场");
    }
}
