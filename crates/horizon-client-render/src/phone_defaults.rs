//! The settings a phone starts with: Horizon's own graphics preset for its RAM class, Dynamic
//! Resolution holding 60 fps, and shorter distances, set once on a fresh install and never over
//! a saved choice.
use crate::{
    graphics::{Preset, apply_preset},
    settings::Settings,
};

/// A phone's RAM, by `/proc/meminfo`'s `MemTotal` (the kernel's share, a little under the box's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RamClass {
    /// 4 GiB or less.
    Low,
    /// Up to 6 GiB.
    Mid,
    High,
}

/// `MemTotal` of `/proc/meminfo` text, in MiB.
pub fn mem_total_mib(meminfo: &str) -> Option<u64> {
    let line = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:"))?;
    let kib: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some(kib / 1024)
}

/// The class of `/proc/meminfo` text; text without a `MemTotal` is taken as Mid.
pub fn ram_class(meminfo: &str) -> RamClass {
    match mem_total_mib(meminfo) {
        Some(mib) if mib <= 4096 => RamClass::Low,
        Some(mib) if mib > 6144 => RamClass::High,
        _ => RamClass::Mid,
    }
}

/// The graphics preset a class starts from.
pub fn preset_for(ram: RamClass) -> Preset {
    match ram {
        RamClass::Low => Preset::FullPerformance,
        RamClass::Mid => Preset::Performance,
        RamClass::High => Preset::Balanced,
    }
}

/// Whether this is a first run: none of the settings files exists. One of them missing while
/// another exists means a player (or an older start) already saved, so nothing is applied.
pub fn is_fresh(options: bool, graphics: bool, extras: bool) -> bool {
    !(options || graphics || extras)
}

/// Sets the fresh-install phone defaults: the class's preset, then Dynamic Resolution toward
/// 60 fps above a floor of 75% on High and 50% on Low and Mid (measured on a Galaxy A14: its
/// GPU is pixel-bound, 30 fps at 75% and 45 at 50%), distances capped at 8 and 6 (6 and 5 on
/// Low), a 60 fps limit, auto-jump and the inventory sort button. Everything else stays as the
/// preset set it.
pub fn apply(ram: RamClass, s: &mut Settings) {
    let low = ram == RamClass::Low;
    apply_preset(preset_for(ram), s);
    s.render_distance = s.render_distance.min(if low { 6 } else { 8 });
    s.simulation_distance = s.simulation_distance.min(if low { 5 } else { 6 });
    s.max_fps = 60;
    s.auto_jump = true;
    s.extras.sort = true;
    let g = &mut s.horizon;
    g.dynamic_resolution = true;
    g.target_fps = 60;
    g.resolution_scale = g.resolution_scale.min(if ram == RamClass::High { 75 } else { 50 });
}

/// Applies the defaults for this phone's RAM (Mid when `/proc/meminfo` can't be read) and says
/// so on stderr.
pub fn apply_for_this_device(s: &mut Settings) {
    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let ram = ram_class(&meminfo);
    apply(ram, s);
    eprintln!(
        "Meridian: phone defaults for {ram:?} ({} MiB): {:?}, render {}, sim {}, dyn-res {} fps from {}%",
        mem_total_mib(&meminfo).unwrap_or(0),
        s.horizon.preset,
        s.render_distance,
        s.simulation_distance,
        s.horizon.target_fps,
        s.horizon.resolution_scale,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meminfo(kib: u64) -> String {
        format!("MemTotal:       {kib} kB\nMemFree:         123456 kB\nMemAvailable:   1234567 kB\n")
    }

    #[test]
    fn ram_class_reads_meminfo() {
        assert_eq!(ram_class(&meminfo(3_812_345)), RamClass::Low);
        assert_eq!(ram_class(&meminfo(4_194_304)), RamClass::Low);
        // The Galaxy A14 5G.
        assert_eq!(ram_class(&meminfo(5_728_944)), RamClass::Mid);
        assert_eq!(ram_class(&meminfo(6_291_456)), RamClass::Mid);
        assert_eq!(ram_class(&meminfo(7_900_000)), RamClass::High);
        assert_eq!(mem_total_mib(&meminfo(5_728_944)), Some(5594));
    }

    #[test]
    fn unreadable_meminfo_is_mid() {
        for text in ["", "garbage", "MemTotal: lots kB", "MemTotal: 12345", "MemFree: 5 kB"] {
            assert_eq!(ram_class(text), RamClass::Mid, "{text:?}");
        }
    }

    #[test]
    fn fresh_only_when_all_three_files_are_missing() {
        assert!(is_fresh(false, false, false));
        for (o, g, e) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert!(!is_fresh(o, g, e));
        }
    }

    #[test]
    fn apply_gives_each_class_its_preset_floor_and_caps() {
        // (class, preset, render, simulation, floor)
        for (ram, preset, render, sim, floor) in [
            (RamClass::Low, Preset::FullPerformance, 6, 5, 50),
            (RamClass::Mid, Preset::Performance, 8, 6, 50),
            (RamClass::High, Preset::Balanced, 8, 6, 75),
        ] {
            let mut s = Settings::default();
            apply(ram, &mut s);
            assert_eq!(preset_for(ram), preset);
            assert_eq!(s.horizon.preset, preset, "{ram:?}");
            assert!(s.horizon.dynamic_resolution, "{ram:?}");
            assert_eq!(s.horizon.target_fps, 60);
            assert_eq!(s.horizon.resolution_scale, floor, "{ram:?}");
            assert_eq!((s.render_distance, s.simulation_distance), (render, sim), "{ram:?}");
            assert_eq!(s.max_fps, 60);
            assert!(s.auto_jump);
            assert!(s.extras.sort);
            assert!(!s.extras.dynamic_lights);
        }
    }

    #[test]
    fn apply_leaves_what_no_rule_names_at_the_presets_values() {
        let mut preset_only = Settings::default();
        apply_preset(Preset::Balanced, &mut preset_only);
        let mut s = Settings::default();
        apply(RamClass::High, &mut s);
        assert_eq!(s.horizon.fxaa, preset_only.horizon.fxaa);
        assert_eq!(s.clouds, preset_only.clouds);
        assert_eq!(s.particles, preset_only.particles);
        assert_eq!(s.biome_blend, preset_only.biome_blend);
        assert_eq!(s.entity_distance, preset_only.entity_distance);
    }

    #[test]
    fn the_defaults_survive_a_save_and_parse() {
        let mut s = Settings::default();
        apply(RamClass::Mid, &mut s);
        let again = Settings::parse(&s.serialize());
        assert_eq!((again.render_distance, again.simulation_distance), (8, 6));
        assert_eq!(again.max_fps, 60);
        assert!(again.auto_jump);
    }
}
