# Gun model passes: shots

Every shot here has all the generated textures (materials, terrain,
foliage, sky); `scripts/fetch_assets.py` builds them, and the game now
refuses a shot without them.

- `retake/<list>_<shot>.png`: each before/after rendered with the two
  commits' own builds (`scripts/build_at.sh`), the gallery staging named
  in `lists/<list>.txt`. They replace the first shots of the PM and AK
  passes, whose backgrounds were taken with only the material textures.
  - `ak_sights_*`: the before build (b2e75828) still had the sights
    gallery's bug that brought every gun in the world to the eye: the
    crossbow over the AK is that bug, fixed in the after.
  - `ak_wood_*`: generated textures are shared between builds, so the
    before half has the wood's new brightness too; the pair shows only
    the finer tiling.
- `mp133_*`, `toz8_*`, `toz_double_*`: from-the-eye checks; nothing to
  change.
- `rpk74_stock_*`: the RPK's stock rounded across.
