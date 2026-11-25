//! Original 22x22 label overlays for the key buttons (normal and pressed), drawn over the Bedrock
//! jump frame: `HudAtlas` loads each as the sprite `gui/meridian/keys/<key>` (and `<key>_pressed`).

macro_rules! icons {
    ($($key:literal)*) => {
        &[$(
            (
                concat!("gui/meridian/keys/", $key),
                include_bytes!(concat!("keys/key_", $key, ".png")) as &[u8],
            ),
            (
                concat!("gui/meridian/keys/", $key, "_pressed"),
                include_bytes!(concat!("keys/key_", $key, "_pressed.png")) as &[u8],
            ),
        )*]
    };
}

/// Each sprite's name and its PNG.
pub(crate) const SPRITES: &[(&str, &[u8])] = icons!(
    "b" "c" "f" "g" "h" "i" "j" "k" "l" "m" "n" "o" "p" "q" "r" "u" "v" "x" "y" "z"
    "f1" "f2" "f4" "f6" "f7" "f8" "f9" "f10" "f11" "f12"
    "settings" "offhand" "inventory"
);

/// The HUD atlas name of `key`'s label sprite.
pub(super) fn sprite(key: &str, pressed: bool) -> String {
    let pressed = if pressed { "_pressed" } else { "" };
    format!("gui/meridian/keys/{}{pressed}", key.to_lowercase())
}

/// Builds single-layer `18×18` (scaled by `gui_w / 256`) top-bar sprites for `"settings"` and
/// `"offhand"` by taking Bedrock's Pause button at `[200, 64, 18, 18]` in `gui.png`, stripping
/// its three `≡` bars to the button's plain face colour, and stamping each `12×12` glyph from
/// `icon_images` in the exact ink colour.
pub(crate) fn top_bar_sprites(
    gui_w: u32,
    gui_h: u32,
    gui_rgba: &[u8],
    icon_images: &[(&str, u32, u32, &[u8])],
) -> Vec<(String, u32, u32, Vec<u8>)> {
    if gui_w < 256
        || gui_h < 256
        || gui_w % 256 != 0
        || gui_h % 256 != 0
        || gui_rgba.len() < (gui_w as usize) * (gui_h as usize) * 4
    {
        return Vec::new();
    }
    let s = (gui_w / 256) as usize;
    let tile = 18 * s;
    let ox = 200 * s;
    let oy = 64 * s;
    if oy + tile > gui_h as usize {
        return Vec::new();
    }
    let gui_stride = (gui_w as usize) * 4;
    let row_bytes = tile * 4;
    let mut result = Vec::new();
    for (key_name, top_name) in [
        ("gui/meridian/keys/settings", "gui/meridian/top/settings"),
        ("gui/meridian/keys/offhand", "gui/meridian/top/offhand"),
    ] {
        let Some(&(_, _, _, irgba)) = icon_images.iter().find(|(n, iw, ih, rgba)| {
            *n == key_name && *iw == 22 && *ih == 22 && rgba.len() >= 22 * 22 * 4
        }) else {
            continue;
        };
        let mut out = vec![0u8; tile * tile * 4];
        for row in 0..tile {
            let src_off = (oy + row) * gui_stride + ox * 4;
            let dst_off = row * row_bytes;
            out[dst_off..dst_off + row_bytes]
                .copy_from_slice(&gui_rgba[src_off..src_off + row_bytes]);
        }
        let px_at = |x: usize, y: usize| -> [u8; 4] {
            let i = (y * tile + x) * 4;
            [out[i], out[i + 1], out[i + 2], out[i + 3]]
        };
        let face_px = px_at(2 * s, 2 * s);
        let ink_px = px_at(8 * s, 4 * s);
        for ly in (2 * s)..(14 * s) {
            for lx in (2 * s)..(16 * s) {
                let i = (ly * tile + lx) * 4;
                out[i..i + 4].copy_from_slice(&face_px);
            }
        }
        for iy in 0..12 {
            for ix in 0..12 {
                if irgba[((4 + iy) * 22 + (5 + ix)) * 4 + 3] >= 128 {
                    for dy in 0..s {
                        for dx in 0..s {
                            let lx = (3 + ix) * s + dx;
                            let ly = (2 + iy) * s + dy;
                            let i = (ly * tile + lx) * 4;
                            out[i..i + 4].copy_from_slice(&ink_px);
                        }
                    }
                }
            }
        }
        result.push((top_name.to_owned(), tile as u32, tile as u32, out));
    }
    result
}

/// Builds the docked and standalone `22×22` (scaled by `hotbar_w / 182`) Bedrock-style inventory
/// hotbar cell sprites from Java's `hotbar.png` and `key_inventory{,_pressed}`.
pub(crate) fn inventory_sprites(
    hotbar_w: u32,
    hotbar_h: u32,
    hotbar_rgba: &[u8],
    icon_images: &[(&str, u32, u32, &[u8])],
) -> Vec<(String, u32, u32, Vec<u8>)> {
    if hotbar_w < 182
        || hotbar_h < 22
        || hotbar_w % 182 != 0
        || hotbar_h % 22 != 0
        || hotbar_w / 182 != hotbar_h / 22
        || hotbar_rgba.len() < (hotbar_w as usize) * (hotbar_h as usize) * 4
    {
        return Vec::new();
    }
    let s = (hotbar_w / 182) as usize;
    let tile = 22 * s;
    let ox = 160 * s;
    let hb_stride = (hotbar_w as usize) * 4;
    let row_bytes = tile * 4;
    let left_bytes = 3 * s * 4;
    let mut result = Vec::new();
    for (key_name, docked_name, standalone_name) in [
        (
            "gui/meridian/keys/inventory",
            "gui/meridian/inventory",
            "gui/meridian/inventory_standalone",
        ),
        (
            "gui/meridian/keys/inventory_pressed",
            "gui/meridian/inventory_pressed",
            "gui/meridian/inventory_standalone_pressed",
        ),
    ] {
        let Some(&(_, _, _, irgba)) = icon_images.iter().find(|(n, iw, ih, rgba)| {
            *n == key_name && *iw == 22 && *ih == 22 && rgba.len() >= 22 * 22 * 4
        }) else {
            continue;
        };
        let mut docked = vec![0u8; tile * tile * 4];
        for row in 0..tile {
            let src_off = row * hb_stride + ox * 4;
            let dst_off = row * row_bytes;
            docked[dst_off..dst_off + row_bytes]
                .copy_from_slice(&hotbar_rgba[src_off..src_off + row_bytes]);
        }
        for ly in 3..19 {
            for lx in 3..19 {
                let src_off = (ly * 22 + lx) * 4;
                let px = &irgba[src_off..src_off + 4];
                for dy in 0..s {
                    for dx in 0..s {
                        let dst_off = ((ly * s + dy) * tile + (lx * s + dx)) * 4;
                        docked[dst_off..dst_off + 4].copy_from_slice(px);
                    }
                }
            }
        }
        let mut standalone = docked.clone();
        for row in 0..tile {
            let src_off = row * hb_stride;
            let dst_off = row * row_bytes;
            standalone[dst_off..dst_off + left_bytes]
                .copy_from_slice(&hotbar_rgba[src_off..src_off + left_bytes]);
        }
        result.push((docked_name.to_owned(), tile as u32, tile as u32, docked));
        result.push((
            standalone_name.to_owned(),
            tile as u32,
            tile as u32,
            standalone,
        ));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key the editor adds and the top-bar icon controls have their sprites in both looks,
    /// except Pick (text).
    #[test]
    fn every_key_button_has_an_icon_but_pick() {
        for key in crate::touch::layout::KEYS
            .iter()
            .copied()
            .filter(|k| *k != "Pick")
            .chain(["settings", "offhand", "inventory"])
        {
            for pressed in [false, true] {
                let name = sprite(key, pressed);
                assert!(SPRITES.iter().any(|(n, _)| *n == name), "{name}");
            }
        }
    }

    #[test]
    fn top_bar_sprites_strip_pause_bars_and_stamp_single_layer_icons() {
        let mut gui = vec![0u8; 256 * 256 * 4];
        let mut put = |x: usize, y: usize, px: [u8; 4]| {
            let i = (y * 256 + x) * 4;
            gui[i..i + 4].copy_from_slice(&px);
        };
        let border = [20, 20, 20, 204];
        let lip = [90, 90, 90, 204];
        let face = [129, 129, 129, 204];
        let bar = [0, 0, 0, 204];
        for ly in 0..18 {
            for lx in 0..18 {
                let px = if lx == 0 || lx == 17 || ly == 0 || ly == 17 {
                    border
                } else if ly >= 14 {
                    lip
                } else {
                    face
                };
                put(200 + lx, 64 + ly, px);
            }
        }
        for ly in [4, 7, 10] {
            for lx in 5..12 {
                put(200 + lx, 64 + ly, bar);
            }
        }
        let decode = |name: &'static str| {
            let bytes = SPRITES.iter().find(|(n, _)| *n == name).unwrap().1;
            let (w, h, rgba) = crate::hud::decode_bytes(bytes).unwrap();
            (name, w, h, rgba)
        };
        let settings = decode("gui/meridian/keys/settings");
        let offhand = decode("gui/meridian/keys/offhand");
        let icons = [
            (settings.0, settings.1, settings.2, settings.3.as_slice()),
            (offhand.0, offhand.1, offhand.2, offhand.3.as_slice()),
        ];
        let out = top_bar_sprites(256, 256, &gui, &icons);
        assert_eq!(out.len(), 2);
        let px_in = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
            let i = (y * 18 + x) * 4;
            [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
        };

        let (name_s, w_s, h_s, buf_s) = &out[0];
        assert_eq!(name_s, "gui/meridian/top/settings");
        assert_eq!((*w_s, *h_s), (18, 18));
        assert_eq!(px_in(buf_s, 0, 0), border);
        assert_eq!(px_in(buf_s, 1, 15), lip);
        for ly in 7..=8 {
            for lx in 8..=9 {
                assert_eq!(px_in(buf_s, lx, ly), face, "gear center ({lx},{ly})");
            }
        }
        for lx in 8..10 {
            assert_eq!(px_in(buf_s, lx, 2), bar, "gear outer tooth ({lx},2)");
        }

        let (name_o, w_o, h_o, buf_o) = &out[1];
        assert_eq!(name_o, "gui/meridian/top/offhand");
        assert_eq!((*w_o, *h_o), (18, 18));
        assert_eq!(px_in(buf_o, 0, 0), border);
        assert_eq!(px_in(buf_o, 1, 15), lip);
        for ly in 7..9 {
            for lx in 3..15 {
                assert_eq!(px_in(buf_o, lx, ly), face, "swap center gap ({lx},{ly})");
            }
        }
        assert_eq!(px_in(buf_o, 10, 2), bar, "swap arrow tip (10,2)");
    }

    #[test]
    fn inventory_sprites_match_hotbar_frame_and_stamp_crisp_dots() {
        let mut hotbar = vec![0u8; 182 * 22 * 4];
        let mut put = |x: usize, y: usize, px: [u8; 4]| {
            let i = (y * 182 + x) * 4;
            hotbar[i..i + 4].copy_from_slice(&px);
        };
        let left_border = [10, 10, 10, 255];
        let divider = [30, 30, 30, 255];
        let right_border = [50, 50, 50, 255];
        let cavity = [70, 70, 70, 255];
        for y in 0..22 {
            for x in 0..182 {
                let px = if x < 3 {
                    left_border
                } else if (160..163).contains(&x) {
                    divider
                } else if x == 181 {
                    right_border
                } else if (3..19).contains(&y) {
                    cavity
                } else {
                    [90, 90, 90, 255]
                };
                put(x, y, px);
            }
        }
        let decode = |name: &'static str| {
            let bytes = SPRITES.iter().find(|(n, _)| *n == name).unwrap().1;
            let (w, h, rgba) = crate::hud::decode_bytes(bytes).unwrap();
            (name, w, h, rgba)
        };
        let normal = decode("gui/meridian/keys/inventory");
        let pressed = decode("gui/meridian/keys/inventory_pressed");
        let icons = [
            (normal.0, normal.1, normal.2, normal.3.as_slice()),
            (pressed.0, pressed.1, pressed.2, pressed.3.as_slice()),
        ];
        let out = inventory_sprites(182, 22, &hotbar, &icons);
        assert_eq!(out.len(), 4);
        let expected_names = [
            "gui/meridian/inventory",
            "gui/meridian/inventory_standalone",
            "gui/meridian/inventory_pressed",
            "gui/meridian/inventory_standalone_pressed",
        ];
        for ((name, w, h, _), expected) in out.iter().zip(expected_names) {
            assert_eq!(name, expected);
            assert_eq!((*w, *h), (22, 22));
        }
        let px_in = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
            let i = (y * 22 + x) * 4;
            [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
        };
        let docked_normal = &out[0].3;
        let standalone_normal = &out[1].3;
        let docked_pressed = &out[2].3;
        assert_eq!(px_in(docked_normal, 0, 10), divider);
        assert_eq!(px_in(docked_normal, 21, 10), right_border);
        assert_eq!(px_in(standalone_normal, 0, 10), left_border);
        assert_eq!(px_in(standalone_normal, 21, 10), right_border);
        for x in [6, 10, 14] {
            assert_eq!(px_in(docked_normal, x, 10), [240, 240, 240, 255]);
        }
        assert_ne!(px_in(docked_pressed, 6, 10), [240, 240, 240, 255]);
        assert_eq!(px_in(docked_pressed, 6, 11), [212, 212, 212, 255]);
    }
}
