# Importing a hand model

What a replacement glove/hand `.glb` must provide for the game's hands to
drive it, and how to bring one in. Written for the user's CC0 hand (to come
in `cass-avatar`), in place of `assets/models/hands/glove_{left,right}.glb`
(`art/hands/gloves.py`). The code named here is `src/render/props.rs` unless
said otherwise; Hands owns the posing, Models the art and `dress_gloves`.

## What the game drives

`spawn_hand` builds each hand as entities, and every pose (the tracked
hand's curls and spread, a gun's grip and trigger fit, held items' seats,
pointing) only ever moves these. A model is dressed onto them
(`dress_gloves`); nothing in the posing reads the model.

- **The hand root** (`HandVisual`), on the tracked hand (its origin the
  controller's grip, the middle of the closed fist; -Z forward, +Y up).
- **The glove frame** (`GloveParts::model`, the `palm`): `glove_frame(side)`
  under the root. Everything below is in this frame:
  - **palm down, fingers along -Z, the back of the hand +Y**;
  - the palm's centre at the origin (the knuckle line at z ≈ -0.048, the
    wrist at z ≈ +0.056, the cuff to z ≈ +0.11);
  - **the thumb on -X for a right hand, +X for a left** (`thumb_side =
    -side.sign()`);
  - metres, 1:1 (the palm ~85 mm across, ~25 mm thick).
  `glove_frame` rolls it onto the hand and shifts it so `GRIP`
  (0, -0.028, -0.041), inside the closed fingers under the knuckles, sits on
  the grip.
- **14 finger joints** (`Finger { finger, joint }`), chained:
  `finger_F_J`, F 0..3 index → little, 4 the thumb; J 0 at the knuckle.
  The index, middle, ring and little have 3 joints, the thumb 2
  (`finger_4_0`, `finger_4_1`).
  - **Segment lengths** (`FINGER_BONES`, m, knuckle outwards): index
    0.040 / 0.025 / 0.020, middle 0.044 / 0.028 / 0.022, ring 0.041 /
    0.026 / 0.020, little 0.033 / 0.020 / 0.018, thumb 0.036 / 0.030.
  - **Joint 0 sits at `finger_root(side, F)`**: fingers at
    x = (0.03 - 0.02 F)·thumb_side, y 0, z -0.048; the thumb at
    (0.038·thumb_side, -0.008, -0.005).
  - **Each next joint** sits (0, 0, -length) along its parent, the bone
    running down the joint's own **-Z**.
  - **Curl is a turn about the joint's X, toward -Y** (the palm):
    `JOINT_BEND` 1.4 / 1.65 / 1.1 rad at full curl; the thumb's second
    joint 0.9 (`THUMB_BEND`). The fingers' first joints also spread about
    Y (`rest_spread`: 0.04 rad a finger away from the index, open). The
    thumb's root swings rather than bends (`finger_aim_spread`): an aim
    from open (beside the index) through on-a-grip to across the fist,
    with a fixed roll of 0.6 rad about its Z.
  - **The rest pose is curl 0, `rest_spread`**: what
    `joint_rotation(side, F, J, 0.0)` gives; joints 1 and 2 are
    unturned at rest, so each finger is straight from its knuckle.

## What a model must provide

1. **One `.glb` per hand**, `glove_left.glb` and `glove_right.glb` (or
   one mirrored at import; the frame is mirrored in X, not rotated).
2. **A skinned mesh, joints named exactly `palm` and `finger_F_J`** (15
   joints), rest-posed at the joints above: each joint node's translation
   and rotation (relative to its parent; `palm` at identity) equal to
   `spawn_hand`'s at rest. The glTF's inverse binds then fit the game's
   joints as they are. (A Blender bone exports turned -90° about its X:
   `art/hands/gloves.py` on the WIP commit 457f45af turns each bone back
   before export, so the nodes land as the joints.)
3. **The mesh in the glove frame**, under a root at identity, at metre
   scale: no scale on the root or the armature.
4. **The index fingertip at `interact::poke::FINGERTIP`** when pointing
   (`POINTING`), i.e. the index's last segment ending 0.020 m out along
   its -Z: a push button presses where the glove touches it
   (`pointing_index_ends_on_the_poke_fingertip`).
5. **Finger radii near `hold.rs`'s `RADII`** (index ~9.4 mm at its root
   to 7.3 at its tip; the thumb 12.2 to 8.6). Held items are seated by
   these radii (`render::hold`: sink at most 4 mm, touch within 3): a
   much fatter or thinner finger sinks into or floats off what it holds.
   If the new hand's are different, change `RADII` with it.
6. **The palm's underside about y -0.013** and its thenar (the ball of
   the thumb) at the thumb's root (`hold::palm_points`, `hold::thenar`):
   what a held thing rests against.
7. **Materials named `Glove`, `Leather` and `Padding`** (`ModelMaterials`
   paints them: nylon back, leather palm and fingertips, dark padding).
   Other names fall back to the library's; the game gives each glove its
   own copies to tint when a hand can reach something (`tint_gloves`).
8. **`wrist_housing`, a separate rigid mesh node** in the glove frame:
   the wrist display's housing on the cuff's back, about 52 x 7 x 82 mm
   centred (0, 0.016, 0.0625). It's shown only on the hand that isn't the
   gun hand (`handed_housing`), and the display itself is laid 4 mm over
   its top face (`wrist_on_glove`: y 0.0235, z 0.0625). Keep the cuff's
   back flat enough there.
9. **The cuff closed at the arm's end** (the arms aren't drawn: an open
   cuff showed a black hole whenever the hand pointed away).

A model without a skin still works: each mesh node named `palm` or
`finger_F_J` is hung rigidly on its joint (`skin()`, the current gloves).

## Importing it

1. Bring the model and its licence (CC0) and source link into
   `assets/models/hands/` (and a note in `docs/ASSETS.md` or beside it).
2. Rename or map its bones to `palm` / `finger_F_J`. A hand rig usually
   has a wrist bone and metacarpals: weight the metacarpals to `palm`, and
   drop or merge a bone the game has no joint for (a fourth thumb bone, the
   wrist) into its neighbour.
3. Re-pose its rest to the joints above: scale to the segment lengths,
   align each knuckle to `finger_root`, open and straight with the fingers
   along -Z, palm down, thumb on the side above. An automated check is
   the props test `every_glove_segment_is_modelled_at_its_joint`
   (translations within 0.2 mm, rotations to 0.9999); it currently also
   asks each joint node for a mesh, and the pointing test reads
   `finger_0_2`'s mesh bounds: for a skinned model, give each joint the
   bounds of the vertices weighted ≥ 0.5 to it (agreed with Hands).
4. Dress it: `dress_gloves` on the WIP commit 457f45af already spawns a
   glTF skin's primitives as `SkinnedMesh` on the joints by name (copying
   the inverse binds into the glove's own asset and generating tangents),
   with the rigid path as its fallback. Take that hunk.
5. Take the WIP's `render/warmup.rs` hunk too: the warm-up shelf skips a
   primitive with joint indices (shelved without its skin, the frame
   failed validation and the game quit).
6. Shoot `--gallery fp:open`, `fp:brass` (a fist), `fp:pm_pistol` and
   `fp:round_9x18` (a pinch) at the same cameras as
   `docs/shots/hands/skinned_glove_wip_*.png`, before and after.

## The paused skinned glove

Commit 457f45af (reverted in the next) holds a skinned glove built from
`gloves.py`: one continuous mesh, each finger a tube from inside the palm
to its tip, weights blended over each joint (9 mm at the knuckles, 13 mm
at the thumb's root, 4.5 mm at the finger joints), 13,056 triangles to a
hand. It renders and poses; its stills are in `docs/shots/hands/` on
that commit. It is a fallback if the imported hand doesn't come.
