# Logical Zoom

Hold C to zoom. The world projection changes immediately, first-person hands and items disappear, and mouse camera movement uses cinematic smoothing for the hold. Bind C to a touch key in Meridian's controls. Releasing the key restores the ordinary view and the previous mouse smoothing setting.

This mod implements the behavior of Logical Zoom 0.0.37 by LogicalGeekBoy. It is independent of the official Horizon Zoom mod; disable that mod to avoid applying both zooms at once. The game limits the returned field of view to at least one degree. The current mod API exposes cinematic smoothing for mouse look only, so touch and controller camera movement are not smoothed by this mod.

## Build and install

From this directory, use the matching Meridian SDK/toolchain and run `cargo horizon-mod build --wasm`. Copy the resulting `target/horizon-mod/logical_zoom.hmod` to the instance's `horizon-mods/` folder and enable `mod.logical_zoom:true` in `horizon-mods.txt`, or enable it in the Mods screen. Restart the game after installing. The packager writes the WASM hash and toolchain into the manifest; do not add them by hand.
