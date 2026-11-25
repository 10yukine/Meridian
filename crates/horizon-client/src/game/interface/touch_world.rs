// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, `crates/horizon-client/src/game/pad.rs` and `game.rs`), written by Vasysik and the
// Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the
// same MIT license as Horizon (LICENSE at the repository root).

//! Meridian: the touch controls act on the world where the finger is. A short tap attacks the
//! entity under it, or uses the hand on the block (places a block); a hold mines the block, or
//! uses the held item (eats, draws a bow) or the entity (a boat, a villager's trades).
#![cfg(any(target_os = "android", target_os = "ios"))]

use super::{Game, Screen};
use horizon_client::{
    actions::Action, first_person::Hand, gamepad::PadState, keymapping::SNEAK,
};
use horizon_client_render::{
    android_gamepad::InputMode,
    android_runtime::{self, active_input},
    touch,
};

impl Game<'_> {
    /// Once a frame: aiming by turning the phone, while the touch page asks for it and the player
    /// plays by touch (the sensor is listened to only then). Its turn goes on the view as a
    /// touch drag's does. After Starling's `touch_gyro`.
    pub(super) fn touch_gyro(&mut self) {
        let settings = &self.pad.touch.settings;
        if !settings.gyro {
            // The setting changed: a phone found without a gyroscope may be asked again.
            self.pad.touch_gyro_missing = false;
        }
        let wanted = settings.gyro
            && !self.pad.touch_gyro_missing
            && self.screen == Screen::Playing
            && self.player.is_some()
            && !self.screen_over_playing()
            && !self.pad.touch.editing()
            && active_input() != InputMode::Controller;
        if wanted != self.pad.touch_gyro_on {
            let started = android_runtime::gyroscope(wanted);
            if wanted && !started {
                log::info!("Meridian: no gyroscope to aim with");
                self.pad.touch_gyro_missing = true;
                return;
            }
            self.pad.touch_gyro_on = wanted;
        }
        if !wanted {
            return;
        }
        let Some([x, y, rotation]) = android_runtime::take_gyro() else {
            return;
        };
        let sensitivity = self.pad.touch.settings.gyro_sensitivity;
        let [yaw, pitch] =
            touch::gyro::gyro_turn([x, y], rotation as i32).map(|degrees| degrees * sensitivity);
        if yaw != 0.0 || pitch != 0.0 {
            // Degrees, as a drag's: the game turns in 0.15 degree units.
            self.look_turn(f64::from(yaw / 0.15), f64::from(pitch / 0.15));
        }
    }

    /// The app leaves the screen: the gyroscope rests (the next frame that aims listens again).
    // ponytail: with the debug `no_pause` option the game keeps playing in the background and
    // the next frame listens again; nobody plays that way on a phone.
    pub(super) fn touch_gyro_stop(&mut self) {
        if std::mem::take(&mut self.pad.touch_gyro_on) {
            android_runtime::gyroscope(false);
        }
    }

    /// Where a world touch aims (0..1 of the surface): the finger, or the last one until the
    /// press it started is let go.
    fn touch_point(&self) -> Option<[f32; 2]> {
        self.pad.touch.aim.or(self.pad.touch_aim)
    }

    /// The ray through the touched point, in place of `forward` (the view), while a touch aims.
    pub(super) fn touch_direction(&self, forward: [f64; 3]) -> [f64; 3] {
        let (Some(point), Some(eye)) = (self.touch_point(), self.eye_position()) else {
            return forward;
        };
        let fov = f64::from(self.fov_degrees(1.0, eye));
        let aspect = self.window.as_ref().map_or(16.0 / 9.0, |w| {
            let size = w.inner_size();
            f64::from(size.width) / f64::from(size.height.max(1))
        });
        let up = super::view_vector(self.yaw, self.pitch - 90.0);
        touch::screen_direction(forward, up, point, aspect, fov)
    }

    /// Before a finger lands: what a hold on the world does with the item in hand.
    pub(super) fn touch_prepare(&mut self) {
        self.pad.touch.projectile_crosshair = self
            .held()
            .is_some_and(|stack| touch::projectile_crosshair(&stack.item));
        self.pad.touch.hold_uses_item = self.touch_item_held_use();
    }

    /// Lets go of a latched sneak (the pad's toggle or the keyboard's), as Bedrock does when
    /// flight starts; the down button then holds sneak by itself.
    fn touch_stand_up(&mut self) {
        if self.toggles.is_down(SNEAK) {
            // The toggle's own release: it flips the latch off and forgets it was the pad's.
            self.toggles.0[SNEAK].pad_toggle();
            self.sync_toggle(SNEAK);
        }
    }

    /// The touch controls' key and action presses, as the game's key bindings take them: an
    /// action by its name, a key button (`keyboard:B`) as that key on a keyboard.
    pub(super) fn touch_actions(&mut self, actions: Vec<(String, bool)>) {
        for (action, down) in actions {
            // A held down button (flying, swimming) holds sneak itself; the pad's sneak action
            // would toggle it.
            if action == touch::HOLD_SNEAK {
                if down {
                    self.touch_stand_up();
                }
                // The toggle state is what `heal_hold_latches` keeps: with no latch (stood up
                // above) this holds sneak while down; `input_holds` keeps it from healing.
                self.toggles.0[SNEAK].set_down(down, false);
                self.sync_toggle(SNEAK);
                continue;
            }
            if let Some(key) = action.strip_prefix(touch::KEYBOARD) {
                let name = format!("key.keyboard.{}", key.to_lowercase());
                let Some(code) = horizon_client::keybinds::key_code(&name) else {
                    log::warn!("Horizon: touch key button {key}: no such key");
                    continue;
                };
                if let Err(error) = self.mapped_key_event(code, down, false) {
                    log::warn!("Horizon: touch key button {key}: {error}");
                }
                // A key for mods as the window's is: their key mappings and `input_key` hear it.
                self.with_user_input(true, |game| {
                    game.socket_key_input(code, if down { "down" } else { "up" });
                });
                continue;
            }
            let action = action.strip_prefix("action:").unwrap_or(&action);
            if let Err(error) = self.pad_action(action, down) {
                log::warn!("Horizon: touch control {action}: {error}");
            }
        }
    }

    /// After the touch controls' poll: a tap acts once where it landed, a hold keeps acting.
    pub(super) fn touch_world(&mut self, state: &mut PadState) {
        // Presses that arrived without a touch event (a reset letting held key buttons go).
        let actions = self.pad.touch.key_events();
        if !actions.is_empty() {
            self.touch_actions(actions);
        }
        // What the controls show: Bedrock's swim, fly and dismount buttons, a held sneak.
        // Creative flight starting: a crouch latched on the ground would sink the player.
        if self.player.as_ref().is_some_and(|p| p.flying) && !self.pad.touch.vertical_movement {
            self.touch_stand_up();
        }
        let player = self.player.as_ref();
        self.pad.touch.vertical_movement = player.is_some_and(|p| p.flying);
        self.pad.touch.swimming = player.is_some_and(|p| p.fluids.was_touching_water);
        self.pad.touch.mounted = self.riding.is_some();
        self.pad.touch.sneaking = self.keys.held(Action::Sneak);
        if let Some(point) = self.pad.touch.aim {
            self.pad.touch_aim = Some(point);
        }
        if self.pad.touch.aim.is_some()
            && self.pad.touch.holding
            && (self.pad.touch.holding_use
                || self.touch_item_held_use()
                || self.touch_entity_accepts_use())
        {
            // Meridian: one use per hold. A held use repeats every 4 ticks (a rod would cast and
            // reel straight back in), so it lets go after 2 unless an item is in use (eating, a
            // drawn bow, a raised shield), which keeps it down until the finger lifts.
            let since = *self.pad.touch_use_since.get_or_insert(self.ticks);
            let down = self.using.is_some() || self.ticks.saturating_sub(since) < 2;
            state.triggers = [if down { 1.0 } else { 0.0 }, 0.0];
            self.pad.touch.latch_use();
        } else {
            self.pad.touch_use_since = None;
        }
        if let Some(held_use) = self.pad.touch.take_tap() {
            if self.entity_targeted() && self.using.is_none() && self.screen == Screen::Playing {
                // The attack lands now, never as a held button: a mob gone by the next tick
                // can't leave the block behind it to mine.
                if self.remote.is_some() {
                    self.remote_start_attack();
                } else {
                    self.attack();
                }
            } else if !held_use {
                // A block placed, or the hand used; an item with a use waits for a hold.
                state.triggers[0] = 1.0;
            } else if self.remote.is_some() {
                // That item's tap punches the air: the arm swings, at nothing.
                self.remote_swing_packet();
            } else {
                self.swing_arm();
            }
        }
        // A shield in either hand goes up while the player crouches, as on Bedrock, and its
        // use skips the block and entity under the crosshair (`RemoteUseWorld`). It lowers while
        // a finger rests on the world, so a tap still places and a hold still mines or eats, and
        // while the main hand's axe, hoe or shovel would change the block under the crosshair.
        self.pad.touch_shield = self.screen == Screen::Playing
            && self.keys.held(Action::Sneak)
            && self.pad.touch.aim.is_none()
            && self.touch_shield_available()
            && !self.touch_transforms_block();
        if self.pad.touch_shield {
            state.triggers[0] = 1.0;
        }
    }

    /// Whether either hand holds a shield (an item that blocks attacks).
    fn touch_shield_available(&self) -> bool {
        self.stack_in(self.selected)
            .into_iter()
            .chain(self.offhand.as_ref())
            .any(|stack| self.blocks_attacks(stack).is_some())
    }

    /// Whether the main hand's axe, hoe or shovel would change the block under the crosshair (a
    /// log stripped, grass tilled). Java's own rule (`playerHasBlockingItemUseIntent`) has an
    /// off-hand shield stop it unless the player sneaks, so crouching is how a player tills or
    /// strips with a shield held: there the crouch's shield stays down, and a tap does it once
    /// (after Starling's `touch_shield_skips_block`, which kept the shield up and let its held use
    /// do it unasked).
    fn touch_transforms_block(&self) -> bool {
        let (Some(stack), Some(hit)) = (self.stack_in(self.selected), self.pick()) else {
            return false;
        };
        self.block_transform(stack, &hit).is_some()
    }

    /// The hand a raised touch shield is in: the off hand's first, as Bedrock holds it.
    pub(super) fn touch_shield_hand(&self) -> Option<Hand> {
        if !self.pad.touch_shield {
            return None;
        }
        let off = self.offhand.as_ref();
        let in_off = off.is_some_and(|stack| self.blocks_attacks(stack).is_some());
        Some(if in_off { Hand::Off } else { Hand::Main })
    }

    /// A world touch's press was let go: the next one aims afresh.
    pub(super) fn touch_released(&mut self) {
        self.pad.touch_aim = None;
    }

    /// Whether a hold uses the held item instead of mining.
    fn touch_item_held_use(&self) -> bool {
        self.held().is_some_and(|stack| {
            if touch::item_uses_on_tap(&stack.item) {
                return false;
            }
            let Some(info) = self.items().get(&stack.item) else {
                return false;
            };
            // Seeds act as blocks (Meridian): a tap plants them, a hold breaks.
            super::remote::item_has_use(info)
                || info.bundle
                || matches!(
                    &*stack.item,
                    "minecraft:firework_rocket" | "minecraft:lead" | "minecraft:name_tag"
                )
                || self.blocks_attacks(stack).is_some()
        })
    }

    /// Whether the entity under a hold takes a use (a boat, a cart, a villager) rather than a hit.
    fn touch_entity_accepts_use(&mut self) -> bool {
        let before = |game: &Self, distance: Option<f64>| {
            distance.is_some_and(|distance| game.before_block(distance))
        };
        if before(self, self.boat_hit().map(|h| h.1))
            || before(self, self.cart_hit().map(|h| h.1))
            || before(self, self.armor_stand_hit().map(|h| h.1))
            || before(self, self.frame_hit().map(|h| h.1))
        {
            return true;
        }
        let Some((i, distance, _)) = self.mob_hit() else {
            return false;
        };
        self.before_block(distance)
            && (self.remote_mob_would_take(i, Hand::Main)
                || self.remote_mob_would_take(i, Hand::Off))
    }
}
