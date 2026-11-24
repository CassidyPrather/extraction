"""The player's gloves, from a real hand: SiroinoSotai 1.0's hands (CC0,
art/hands/source/siroino-body.blend; CREDITS.md), cut at the wrist,
re-posed onto the game's joints and gloved. Writes
assets/models/hands/glove_{left,right}.glb.

Built in the glove's own frame, the one `spawn_hand` in
src/render/props.rs builds its joints in: palm down, fingers along -Z, the
back of the hand +Y, the palm's centre at the origin, the thumb on -X for a
right hand (+X for a left), metres. docs/HANDS_IMPORT.md is the contract.

How the hand is brought in:

1. Cut: every vertex of the body weighted at least half to the hand's
   bones (`Hand_*` and below); the largest piece is kept, its wrist
   capped.
2. Turned into the glove's frame (the body stands Z up in a T-pose, its
   hands palm down, fingers out along X).
3. Re-posed, per vertex, by its bone weights: each source bone is mapped
   onto the game's joint it becomes (`MAP`), its head onto the joint, its
   axis onto the joint's -Z, stretched to the joint's length
   (`FINGER_BONES`); the palm (`Hand_*`) is scaled to put its knuckles on
   the game's knuckle line and its wrist at z 0.056, and its underside up
   to y -0.013 (where held things rest, `hold::palm_points`). The thumb's
   third bone is folded into its second: the game's thumb has two joints.
4. Gloved: puffed 1.2 mm along its normals, smoothed once (Catmull-Clark),
   painted `Glove` (the nylon back) and `Leather` (the palm, the
   undersides and fingertips; its own knuckles are what the back is read
   by now), and given the procedural glove's cuff (`_gloves_procedural`),
   padded at its strap, closed at the arm.
5. Skinned to `glove_rig`, whose bones rest exactly at `spawn_hand`'s
   joints (`palm`, `finger_F_J`), and exported with `wrist_housing` (the
   wrist display's) as a rigid node.

The procedural gloves (`_gloves_procedural.py`) are the fallback: run on
their own they write the same files.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import bmesh
import bpy
from common import (Part, RY, RZ, S, T, TO_BLENDER, box_uv, export, fillet, main_guard, make,
                    parent, report)
from mathutils import Matrix, Vector

import _gloves_procedural as proc

SOURCE = os.path.join(HERE, "source", "siroino-body.blend")
BODY, RIG = "siroino-full", "siroino-armature"

# Kept equal to src/render/props.rs (FINGER_BONES, finger_root, finger_aim).
FINGER_BONES = proc.FINGER_BONES
FAN = proc.FAN
Z_WRIST = 0.056
# Where the bare palm's underside goes: puffed and smoothed, the glove's
# comes to about y -0.013 (`hold::palm_points`).
PALM_UNDER = -0.0095
# How much fuller than the source's each finger segment is (knuckle
# outwards; the thumb's two): SiroinoSotai's are slim and come to points,
# a gloved finger is round to its tip (`hold.rs` RADII, 9.4 mm at the
# index's root to 7.3 at its tip).
WIDTH = [1.25, 1.45, 1.65]
# The thumb narrowed toward its tip: full, it read as a flat paddle wider
# than the fingers from the eye.
THUMB_WIDTH = [0.9, 1.1]
PUFF = 0.0012         # the glove's thickness over the hand
# The source's bones (`_L`/`_R` appended) onto the game's joints; the
# thumb's distal folds into its intermediate (`finger_4_1`).
MAP = {"Hand": "palm"}
for f, name in enumerate(("Index", "Middle", "Ring", "Little")):
    for j, part in enumerate(("Proximal", "Intermediate", "Distal")):
        MAP[f"{name}{part}"] = f"finger_{f}_{j}"
MAP["ThumbProximal"] = "finger_4_0"
MAP["ThumbIntermediate"] = "finger_4_1"
MAP["ThumbDistal"] = "finger_4_1"


def finger_root(f, ts):
    return Vector(proc.finger_root(f, ts))


def finger_aim(f, ts):
    """The first joint's rotation with the hand open (`finger_aim_spread`
    at curl 0, `rest_spread`)."""
    if f == 4:
        d = Vector((0.45 * ts, -0.25, -0.86)).normalized()
        return Vector((0, 0, -1)).rotation_difference(d).to_matrix().to_4x4() @ RZ(-0.6 * ts)
    return RY(f * FAN * ts)


def chain(f, ts):
    """Each joint's rest matrix in the glove's frame, knuckle outwards."""
    bones = [b for b in FINGER_BONES[f] if b > 0]
    m = T(*finger_root(f, ts)) @ finger_aim(f, ts)
    out = []
    for b in bones:
        out.append(m)
        m = m @ T(0.0, 0.0, -b)
    return out


def rig(ts):
    """The armature: the palm at the glove's origin, each finger's joints
    at `spawn_hand`'s rest joints, each bone's frame its joint's (so the
    glTF's joint nodes read as the game's joints)."""
    data = bpy.data.armatures.new("glove_rig")
    arm = bpy.data.objects.new("glove_rig", data)
    bpy.context.scene.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")

    def bone(name, game, par):
        eb = data.edit_bones.new(name)
        eb.head = (0.0, 0.0, 0.0)
        eb.tail = (0.0, 0.01, 0.0)
        # (A Blender bone runs along its own Y; the exporter turns each
        # bone's frame -90 degrees about X to write it: turned back first,
        # the joint node lands as the game's joint.)
        eb.matrix = (TO_BLENDER @ game @ TO_BLENDER.inverted()
                     @ Matrix.Rotation(math.pi / 2, 4, "X"))
        if par is not None:
            eb.parent = par
        return eb

    palm_b = bone("palm", Matrix.Identity(4), None)
    for f in range(5):
        prev = palm_b
        for j, m in enumerate(chain(f, ts)):
            prev = bone(f"finger_{f}_{j}", m, prev)
    bpy.ops.object.mode_set(mode="OBJECT")
    return arm


# --- Reading the source -------------------------------------------------------

def load_source():
    with bpy.data.libraries.load(SOURCE, link=False) as (src, dst):
        dst.objects = [n for n in src.objects if n in (BODY, RIG)]
    body = next(o for o in dst.objects if o.name.startswith(BODY))
    rig_ = next(o for o in dst.objects if o.name.startswith(RIG))
    return body, rig_


def to_glove(sfx):
    """The source's world (Z up, T-pose) to the glove's frame (game axes):
    the fingers (out along +X on the left, -X on the right) to -Z, the back
    of the hand (+Z) to +Y, the thumb's side (-Y) to the thumb's (+X on the
    left, -X on the right). A rotation either way."""
    if sfx == "_L":
        return Matrix(((0, -1, 0), (0, 0, 1), (-1, 0, 0)))
    return Matrix(((0, 1, 0), (0, 0, 1), (1, 0, 0)))


def bone_frames(rig_, sfx, R):
    """Each source bone's head, tail (in the glove's axes) by its name."""
    out = {}
    for b in rig_.data.bones:
        if b.name.endswith(sfx):
            out[b.name[: -len(sfx)]] = (R @ b.head_local, R @ b.tail_local)
    return out


def frame(axis, up):
    """An orthonormal frame (columns: side, up, -axis) like a joint's, its
    bone down its -Z."""
    z = -axis.normalized()
    y = (up - up.project(z)).normalized()
    x = y.cross(z)
    return Matrix((x, y, z)).transposed()


# --- The re-pose ------------------------------------------------------------------

class Affine:
    def __init__(self, lin, t):
        self.lin, self.t = lin, t

    def __call__(self, p):
        return self.lin @ p + self.t


def bone_map(head, tail, up, tgt, length, width=1.0, reach=None, off=None):
    """A source bone (head, tail, its up) onto a target joint (its 4x4 in
    the glove's frame), stretched along to `length`: its tail there, or,
    given `reach` (how far its skin runs past its head), its skin's end
    there under the glove's puff (a fingertip ends on the poke)."""
    src = frame(tail - head, up)
    dst = tgt.to_3x3()
    k = (length / (tail - head).length if reach is None
         else (length - PUFF) / reach)
    # Its skin centred on the joint's axis (`off`: how far the skin's
    # middle stands off the source bone, square to it): held things are
    # seated by the fingers' radii about their joints (`hold.rs`).
    if off is not None:
        head = head + off
        tail = tail + off
    stretch = src @ Matrix(((width, 0, 0), (0, width, 0), (0, 0, k))) @ src.transposed()
    lin = dst @ src.transposed() @ stretch
    return Affine(lin, tgt.translation - lin @ head)


def thumb_up(frames):
    """The thumb's nail side: away from where its chain bends (toward the
    pad), square to its first bone."""
    h0, t0 = frames["ThumbProximal"]
    h1, t1 = frames["ThumbIntermediate"]
    d0, d1 = (t0 - h0).normalized(), (t1 - h1).normalized()
    bend = d1 - d1.project(d0)
    if bend.length < 1e-6:
        return Vector((0, 1, 0))
    return -bend.normalized()


def maps_for(frames, palm_pts, ts, reach, offs):
    """Each source bone's map onto the game's joints."""
    maps = {}
    # The palm: scaled per axis in the glove's frame. Across: the four
    # knuckles onto the game's knuckle line (least squares); along: the
    # wrist to Z_WRIST and the knuckles' mean to -0.048; up: the knuckles'
    # height to y 0 (the fingers' axes), the back scaled 0.8 over it and
    # the underside brought up to PALM_UNDER.
    knuckles = [frames[f"{n}Proximal"][0] for n in ("Index", "Middle", "Ring", "Little")]
    xs = [k.x for k in knuckles]
    gx = [finger_root(f, ts).x for f in range(4)]
    mx, mg = sum(xs) / 4, sum(gx) / 4
    sx = (sum((a - mx) * (b - mg) for a, b in zip(xs, gx))
          / sum((a - mx) ** 2 for a in xs))
    tx = mg - sx * mx
    wrist = frames["Hand"][0]
    kz = sum(k.z for k in knuckles) / 4
    sz = (-0.048 - Z_WRIST) / (kz - wrist.z)
    tz = Z_WRIST - sz * wrist.z
    ky = sum(k.y for k in knuckles) / 4
    under = min(p.y for p in palm_pts)
    s_dn = PALM_UNDER / (under - ky)
    s_up = 1.3

    def palm(p):
        dy = p.y - ky
        z = sz * p.z + tz
        # Fuller across the middle of the palm than at its knuckles and
        # wrist (the source's narrows behind its knuckles).
        t = min(max((z + 0.048) / (Z_WRIST + 0.048), 0.0), 1.0)
        full = 1.0 + 0.18 * math.sin(math.pi * t)
        return Vector(((sx * p.x + tx) * full, dy * (s_up if dy > 0 else s_dn), z))

    maps["Hand"] = palm
    for f, n in enumerate(("Index", "Middle", "Ring", "Little")):
        joints = chain(f, ts)
        for j, part in enumerate(("Proximal", "Intermediate", "Distal")):
            h, t = frames[n + part]
            maps[n + part] = bone_map(h, t, Vector((0, 1, 0)), joints[j], FINGER_BONES[f][j],
                                      WIDTH[j], reach.get(n + part), offs.get(n + part))
    joints = chain(4, ts)
    up = thumb_up(frames)
    h0, t0 = frames["ThumbProximal"]
    maps["ThumbProximal"] = bone_map(h0, t0, up, joints[0], FINGER_BONES[4][0], THUMB_WIDTH[0],
                                     off=offs.get("ThumbProximal"))
    # Its intermediate and distal as one segment, head to tip.
    h1 = frames["ThumbIntermediate"][0]
    t2 = frames["ThumbDistal"][1]
    m = bone_map(h1, t2, up, joints[1], FINGER_BONES[4][1], THUMB_WIDTH[1],
                 reach.get("ThumbIntermediate"), offs.get("ThumbIntermediate"))
    maps["ThumbIntermediate"] = m
    maps["ThumbDistal"] = m
    return maps


# --- Cutting, re-posing, gloving ---------------------------------------------------

def cut_hand(body, rig_, sfx):
    """The hand's vertices and faces (source world coordinates) and each
    vertex's weights over the hand's bones (source names, no suffix)."""
    hand = {b.name for b in rig_.data.bones if b.name.endswith(sfx)
            and (b.name == "Hand" + sfx
                 or any(p.name == "Hand" + sfx for p in b.parent_recursive))}
    groups = {g.index: g.name for g in body.vertex_groups}
    me = body.data
    weights, keep = {}, set()
    for v in me.vertices:
        ws = {}
        for g in v.groups:
            n = groups[g.group]
            if n in hand and g.weight > 0:
                ws[n[: -len(sfx)]] = g.weight
        if sum(ws.values()) >= 0.5:
            tot = sum(ws.values())
            weights[v.index] = {k: w / tot for k, w in ws.items()}
            keep.add(v.index)
    faces = [p.vertices[:] for p in me.polygons if all(i in keep for i in p.vertices)]
    co = {i: me.vertices[i].co.copy() for i in keep}
    # The ball of the thumb stays on the palm: the source's ThumbProximal
    # runs from the wrist (its metacarpal), but the game's thumb swings from
    # its root in the thenar, and `hold.rs` takes the thenar as the palm's,
    # still. Its weight hands over to the palm along its first two thirds.
    b = rig_.data.bones["ThumbProximal" + sfx]
    h, t = b.head_local, b.tail_local
    d = t - h
    for i in keep:
        w = weights[i]
        tp = w.get("ThumbProximal", 0.0)
        if tp <= 0:
            continue
        u = min(max((co[i] - h).dot(d) / d.length_squared, 0.0), 1.0)
        k = 1.0 - smooth((u - 0.15) / 0.5)
        w["ThumbProximal"] = tp * (1 - k)
        w["Hand"] = w.get("Hand", 0.0) + tp * k
    return co, faces, weights


def smooth(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3 - 2 * t)


def build(side):
    ts = 1.0 if side == "left" else -1.0
    sfx = "_L" if side == "left" else "_R"
    body, rig_ = load_source()
    R = to_glove(sfx)
    frames = bone_frames(rig_, sfx, R)
    co, faces, weights = cut_hand(body, rig_, sfx)
    # The palm's own vertices (mostly `Hand`), for its underside.
    palm_pts = [R @ co[i] for i, w in weights.items() if w.get("Hand", 0) > 0.9]
    # How far each fingertip's skin runs past its last joint (the thumb's
    # from its intermediate's head, the two folded into one).
    reach = {}
    for n in ("Index", "Middle", "Ring", "Little", "Thumb"):
        last = n + "Distal"
        head = frames[n + ("Intermediate" if n == "Thumb" else "Distal")][0]
        d = (frames[last][1] - frames[last][0]).normalized()
        ext = [(R @ co[i] - head).dot(d) for i, w in weights.items() if w.get(last, 0) > 0.5]
        reach[n + ("Intermediate" if n == "Thumb" else "Distal")] = max(ext)
    # How far each finger bone's skin stands off it, square to it.
    offs = {}
    for b, (h, t) in frames.items():
        if b == "Hand" or not any(w.get(b, 0) > 0.5 for w in weights.values()):
            continue
        # (The middle of its bounds across the bone, not their mean: the
        # mesh is denser on one side than the other.)
        f = frame(t - h, Vector((0, 1, 0)) if b[:5] != "Thumb" else thumb_up(frames))
        sx_, sy_ = f.col[0], f.col[1]
        pts = [R @ co[i] - h for i, w in weights.items() if w.get(b, 0) > 0.5]
        a = [p.dot(sx_) for p in pts]
        c = [p.dot(sy_) for p in pts]
        offs[b] = sx_ * (min(a) + max(a)) / 2 + sy_ * (min(c) + max(c)) / 2
    offs["ThumbIntermediate"] = offs.get("ThumbIntermediate", Vector())
    maps = maps_for(frames, palm_pts, ts, reach, offs)

    bm = bmesh.new()
    deform = bm.verts.layers.deform.verify()
    names = sorted({MAP[k] for k in MAP})
    index = {}
    for i, p in co.items():
        g = R @ p
        w = weights[i]
        out = Vector((0, 0, 0))
        for b, wb in w.items():
            out += maps[b](g) * wb
        v = bm.verts.new(TO_BLENDER.to_3x3() @ out)
        for b, wb in w.items():
            gi = names.index(MAP[b])
            v[deform][gi] = v[deform].get(gi, 0.0) + wb
        index[i] = v
    for f in faces:
        try:
            bm.faces.new([index[i] for i in f])
        except ValueError:
            pass
    bm.verts.ensure_lookup_table()
    # The largest connected piece only (a stray vertex or two weighted to
    # the hand up the forearm).
    seen, pieces = set(), []
    for v in bm.verts:
        if v in seen:
            continue
        stack, piece = [v], []
        seen.add(v)
        while stack:
            a = stack.pop()
            piece.append(a)
            for e in a.link_edges:
                b = e.other_vert(a)
                if b not in seen:
                    seen.add(b)
                    stack.append(b)
        pieces.append(piece)
    pieces.sort(key=len)
    for piece in pieces[:-1]:
        bmesh.ops.delete(bm, geom=piece, context="VERTS")
    # Cap the wrist: fill its open ring, a fan to its middle.
    edges = [e for e in bm.edges if e.is_boundary]
    res = bmesh.ops.holes_fill(bm, edges=edges, sides=0)
    bmesh.ops.poke(bm, faces=res["faces"])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("glove")
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new("glove", me)
    bpy.context.scene.collection.objects.link(obj)
    for n in names:
        obj.vertex_groups.new(name=n)
    # The cap's middle vertices have no weight: the palm's.
    palm_g = obj.vertex_groups["palm"]
    for v in me.vertices:
        if not v.groups:
            palm_g.add([v.index], 1.0, "REPLACE")
    # Its source scene objects go.
    for o in (body, rig_):
        bpy.data.objects.remove(o, do_unlink=True)
    glove_it(obj, ts)
    return obj, ts


TIP_R = {}  # each fingertip's pad's radius, gloved, by its group


def round_tips(obj, ts, rounds=6, bins=16):
    """Each fingertip one rounded pad: the source's have nails, a lip round
    the tip that the stretch drew out into a notch. Over its last joint's
    outer half, lightly smoothed (the lip goes), then set onto its own
    section where that half begins (by angle round the joint's axis), kept
    to a hemispherical cap that ends on the segment's length under the
    glove's puff (pointing ends on `FINGERTIP`)."""
    to_game = TO_BLENDER.inverted().to_3x3()
    to_b = TO_BLENDER.to_3x3()
    gi = {g.name: g.index for g in obj.vertex_groups}
    lasts = {}
    for f in range(5):
        bones = [b for b in FINGER_BONES[f] if b > 0]
        j = len(bones) - 1
        lasts[gi[f"finger_{f}_{j}"]] = (chain(f, ts)[j], bones[j])
    me = obj.data
    tips = {}
    for v in me.vertices:
        for g in v.groups:
            if g.group in lasts and g.weight >= 0.5:
                m, L = lasts[g.group]
                if (m.inverted() @ (to_game @ v.co)).z < -0.5 * L:
                    tips.setdefault(g.group, []).append(v.index)
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    for g, idx in tips.items():
        m, L = lasts[g]
        inv = m.inverted()
        verts = [bm.verts[i] for i in idx]
        for _ in range(rounds):
            bmesh.ops.smooth_vert(bm, verts=verts, factor=0.5, use_axis_x=True,
                                  use_axis_y=True, use_axis_z=True)
        qs = [inv @ (to_game @ v.co) for v in verts]
        z0 = -0.5 * L
        # Its section where the tip begins: its middle, and its radius by
        # angle about that middle.
        ring = [q for q in qs if q.z > z0 - 0.002] or qs
        cx = (min(q.x for q in ring) + max(q.x for q in ring)) / 2
        cy = (min(q.y for q in ring) + max(q.y for q in ring)) / 2
        rad = [[] for _ in range(bins)]
        for q in ring:
            a = math.atan2(q.y - cy, q.x - cx)
            rad[int((a + math.pi) / (2 * math.pi) * bins) % bins].append(
                math.hypot(q.x - cx, q.y - cy))
        mean = sum(sum(r) for r in rad) / max(1, sum(len(r) for r in rad))
        rad = [max(r) if r else mean for r in rad]

        def r_at(a):
            t = (a + math.pi) / (2 * math.pi) * bins - 0.5
            k = math.floor(t)
            u = t - k
            return rad[k % bins] * (1 - u) + rad[(k + 1) % bins] * u

        z_tip = -L + PUFF
        R = min(mean, z0 - z_tip)
        TIP_R[g] = R + PUFF
        zc = z_tip + R
        z_min = min(q.z for q in qs)

        def onto(q):
            """A point onto the pad: behind the cap's middle, out to the
            section; past it, onto the cap by its direction from the middle
            (not by how far along it is: the nail overhangs the pad, and so
            ordered it came out as two lumps)."""
            a = math.atan2(q.y - cy, q.x - cx)
            ra = r_at(a)
            if q.z >= zc:
                return Vector((cx + ra * math.cos(a), cy + ra * math.sin(a), q.z))
            d = Vector(((q.x - cx) / ra, (q.y - cy) / ra, (q.z - zc) / R))
            if d.length < 1e-9:
                d = Vector((0, 0, -1))
            d.normalize()
            return Vector((cx + d.x * ra, cy + d.y * ra, zc + d.z * R))

        # Along first: from where it began to the tip's end.
        for v, q in zip(verts, qs):
            q = Vector((q.x, q.y, z_tip + (q.z - z_min) * (z0 - z_tip) / max(z0 - z_min, 1e-6)))
            v.co = to_b @ (m @ onto(q))
        for _ in range(6):
            bmesh.ops.smooth_vert(bm, verts=verts, factor=0.5, use_axis_x=True,
                                  use_axis_y=True, use_axis_z=True)
            for v in verts:
                v.co = to_b @ (m @ onto(inv @ (to_game @ v.co)))
    bm.to_mesh(me)
    bm.free()


def glove_it(obj, ts):
    """Round its tips, puff it, smooth it once, paint it a glove."""
    round_tips(obj, ts)
    me = obj.data
    me.update()
    for v in me.vertices:
        v.co += v.normal * PUFF
    sub = obj.modifiers.new("smooth", "SUBSURF")
    sub.levels = sub.render_levels = 1
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.modifier_apply(modifier="smooth")
    me = obj.data
    for m in ("Glove", "Leather", "Padding"):
        me.materials.append(material(m))
    gi = {g.name: g.index for g in obj.vertex_groups}
    tips = {gi[f"finger_{f}_2"] for f in range(4)} | {gi["finger_4_1"]}
    to_game = TO_BLENDER.inverted().to_3x3()
    for p in me.polygons:
        n = to_game @ p.normal
        c = to_game @ p.center
        ws = {}
        for i in p.vertices:
            for g in me.vertices[i].groups:
                ws[g.group] = ws.get(g.group, 0.0) + g.weight / len(p.vertices)
        top = max(ws, key=ws.get) if ws else None
        mat = 0
        if n.y < -0.3:
            mat = 1  # the leather palm and the fingers' undersides
        elif top in tips and ws[top] > 0.6:
            mat = 1  # worn through first: the fingertips, leather all round
        p.material_index = mat
        p.use_smooth = True
    # Its own UVs, the kit's box projection in metres (the source's were
    # the body's texture atlas: the cloth's texture smeared along them).
    box_uv(me, obj.location)


def material(name):
    from common import MATERIALS
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    rgb, metal, rough, _ = MATERIALS[name]
    m.use_nodes = True
    b = m.node_tree.nodes.get("Principled BSDF")
    b.inputs["Base Color"].default_value = (*rgb, 1.0)
    b.inputs["Metallic"].default_value = metal
    b.inputs["Roughness"].default_value = rough
    return m


def wrist_section(obj, z0=0.040, z1=0.052):
    """The gloved wrist's (half width, centre y, half height) between z0
    and z1 (the cuff goes over it)."""
    to_game = TO_BLENDER.inverted().to_3x3()
    pts = [to_game @ v.co for v in obj.data.vertices]
    ring = [p for p in pts if z0 < p.z < z1]
    xs = [p.x for p in ring]
    ys = [p.y for p in ring]
    return (max(xs) - min(xs)) / 2, (max(ys) + min(ys)) / 2, (max(ys) - min(ys)) / 2


def cuff(obj, ts):
    """The procedural glove's cuff over the wrist, sized to this one's:
    a short gauntlet with a padded strap and its tab, closed at the arm."""
    w, yc, h = wrist_section(obj)
    p = Part("cuff")
    segs, power = 32, proc.POWER
    e = 2.0 / power

    def point(th, grow):
        c, s = math.cos(th), math.sin(th)
        return Vector((math.copysign(abs(c) ** e, c) * (w + grow),
                       yc + math.copysign(abs(s) ** e, s) * (h + grow), 0.0))

    # Its hand end a rolled hem, standing 2.5 mm proud of the glove and lapping
    # 10 mm on over the leather, its edge following the wrist's oval
    # (further toward the fingers at the middle of the palm than at the
    # sides): butted on in a flat ring, the leather palm ended
    # in a dead-straight cut across the wrist (the 22 November playtest,
    # 1:02, "wrist seam").
    HEM = 0.020
    prof = [  # (z, grow over the wrist, scale about its centre)
        (HEM - 0.0006, 0.0015, 1), (HEM, 0.0024, 1), (HEM + 0.0016, 0.0029, 1),
        (HEM + 0.0034, 0.0027, 1), (HEM + 0.0048, 0.0018, 1),
        (0.034, 0.0016, 1), (0.040, 0.0020, 1), (0.050, 0.0026, 1), (0.056, 0.0028, 1),
        (0.0565, 0.0050, 1), (0.0585, 0.0056, 1), (0.0765, 0.0058, 1), (0.0785, 0.0052, 1),
        (0.079, 0.0030, 1), (0.090, 0.0034, 1),
        (0.0935, 0.0034, 0.985), (0.0968, 0.0034, 0.940), (0.1000, 0.0034, 0.866),
        (0.1029, 0.0034, 0.766), (0.1053, 0.0034, 0.643), (0.1073, 0.0034, 0.500),
        (0.1088, 0.0034, 0.342), (0.1097, 0.0034, 0.174), (0.1100, 0.0034, 0.000),
    ]
    centre = Vector((0.0, yc, 0.0))

    def dip(th, z):
        """How far the edge reaches on toward the fingers at angle `th`
        (most at the palm's middle; not on the back, where the display's
        housing stands behind it), fading out up the cuff."""
        fade = min(max((0.046 - z) / (0.046 - HEM), 0.0), 1.0)
        return -0.006 * max(0.0, -math.sin(th)) ** 1.5 * fade

    # Each ring sized to the hand where it goes over it: its section there
    # (the middle, width and height of its skin within 5 mm, 1.5 mm over it:
    # 3 mm over it flared it like a gauntlet), the wrist's
    # further up. Sized to the wrist alone, the hand stood through the
    # cuff's front (a hard edge) and the hem sank into the leather but for
    # a notch where it dipped.
    to_game = TO_BLENDER.inverted().to_3x3()
    skin = [to_game @ v.co for v in obj.data.vertices]
    wrist = (0.0, w, yc, h)

    def section(z):
        near = [q for q in skin if abs(q.z - z) < 0.005]
        if z >= 0.046 or len(near) < 12:
            return wrist
        xs = [q.x for q in near]
        ys = [q.y for q in near]
        sec = ((max(xs) + min(xs)) / 2, (max(xs) - min(xs)) / 2 + 0.0015,
               (max(ys) + min(ys)) / 2, (max(ys) - min(ys)) / 2 + 0.0015)
        # Never narrower than the hand there or the wrist; its middle eased
        # into the wrist's up the cuff.
        t = min(max((z - HEM) / (0.046 - HEM), 0.0), 1.0)
        return (sec[0] * (1 - t), max(sec[1], w), sec[2] * (1 - t) + yc * t, max(sec[3], h))

    def at(th, z, zr, g, sc):
        cx, ww, cy, hh = section(zr)
        c, s_ = math.cos(th), math.sin(th)
        q = Vector((math.copysign(abs(c) ** e, c) * (ww + g),
                    math.copysign(abs(s_) ** e, s_) * (hh + g), 0.0))
        return Vector((cx, cy, z)) + q * sc

    rings = [[at(th, z + dip(th, z), z, g, sc)
              for th in (2 * math.pi * k / segs for k in range(segs))] for z, g, sc in prof]
    _, faces = p.loft(rings, "Glove")
    for f in faces:
        c = f.calc_center_median()
        if 0.0563 < c.z < 0.0787 and f.normal.z * f.normal.z < 0.5:
            f.material_index = p._mi("Padding")
        # The hem, in the padding's dark: its first 5 mm behind the edge
        # (wherever the edge dips to).
        elif c.z - dip(math.atan2(c.y - yc, c.x), HEM) < HEM + 0.0052:
            f.material_index = p._mi("Padding")
    # The strap's tab on the little finger's side, sewn down at its end.
    x = -ts * (w + 0.0055)
    p.box((0.0055, 0.016, 0.02), (x - ts * 0.0012, yc + 0.003, 0.0675), "Padding",
          bevel=0.0018, segs=2)
    c = make(p)
    c.vertex_groups.new(name="palm").add([v.index for v in c.data.vertices], 1.0, "REPLACE")
    # The hand's own skin under the cuff drawn in toward the wrist's middle
    # (up to 20%, from 1 mm behind the hem's edge over 5 mm), so none of it
    # can stand through the cuff (the thumb's root did, at a corner).
    to_b = TO_BLENDER.to_3x3()
    for v in obj.data.vertices:
        q = to_game @ v.co
        k = smooth((q.z - (HEM + 0.001)) / 0.005)
        if k > 0:
            f = 1.0 - 0.2 * k
            v.co = to_b @ Vector((q.x * f, yc + (q.y - yc) * f, q.z))
    # Its UVs projected again where it now lies (drawn in, the weave's
    # projection from before pulled into streaks past the housing).
    box_uv(obj.data, obj.location)
    return c


def radii(obj, ts):
    """The gloved fingers' radii as `hold.rs` RADII lays them out (at each
    joint, the median about the joint's axis of the skin within 2 mm of
    it; then the tip's pad's), printed for Hands to match."""
    to_game = TO_BLENDER.inverted().to_3x3()
    gi = {g.index: g.name for g in obj.vertex_groups}
    pts = [(to_game @ v.co, {gi[g.group]: g.weight for g in v.groups}) for v in obj.data.vertices]
    out = []
    for f in range(5):
        bones = [b for b in FINGER_BONES[f] if b > 0]
        joints = chain(f, ts)
        row = []
        for j in range(len(bones)):
            jj = min(j, len(bones) - 1)
            inv = joints[jj].inverted()
            at = 0.0 if j < len(bones) else -(bones[jj] - 0.0025)
            # (At the knuckle the palm's skin too, near this finger's axis.)
            mine = {f"finger_{f}_{jj}"} | ({f"finger_{f}_{jj - 1}"} if 0 < j < len(bones)
                                           else {"palm"} if j == 0 else set())
            rs = sorted(r for r, q, w in (((q.x ** 2 + q.y ** 2) ** 0.5, q, w)
                                          for q, w in ((inv @ p, w) for p, w in pts))
                        if sum(w.get(n, 0.0) for n in mine) >= 0.5 and abs(q.z - at) < 0.002
                        and (j > 0 or r < 0.013))
            row.append(round(rs[len(rs) // 2] * 1000, 1) if rs else None)
        # The tip: its pad's radius (the end sphere's, as RADII's is).
        row.append(round(TIP_R[obj.vertex_groups[f"finger_{f}_{len(bones) - 1}"].index] * 1000, 1))
        out.append(row)
        print(f"  RADII {f} (mm): {row}")
    return out


def housing():
    """The wrist display's housing, strapped over the cuff's back: its own
    node (`wrist_housing`), drawn only on the hand that isn't the gun hand
    (`render::props`). Its top where the panel lies 4 mm over it (y 0.0195,
    `wrist_on_glove`), as long and wide as the panel needs
    (`render::comfort::WRIST_SIZE`, 78 x 48 mm); its ends sloped and
    rounded down onto the cuff and its long edges rounded, a watch's case
    (with square ends it read as a slab sat on the cuff, playtest
    2026-11-22, 1:02). Inside z 0.021-0.1105 (its rounded front past the
    panel's, 0.0235): the sleeve's hem stops at
    0.112 to clear it."""
    p = Part("wrist_housing")
    y0, y1 = 0.0125, 0.0195
    # (Its front foot just behind the cuff's hem, its rounded front edge
    # over the hem's roll like a watch case's: sloped down onto the hem,
    # it met it in a flat face.)
    side = fillet([(0.0262, y0), (0.1105, y0), (0.1025, y1), (0.0205, y1)],
                  [0.0015, 0.002, 0.003, 0.003], 4)
    p.prism(side, -0.026, 0.026, "Padding", axis="x", bevel=0.0025, segs=3)
    # Its strap across the wrist, under it.
    p.box((0.058, 0.004, 0.012), (0.0, 0.0155, 0.0625), "Padding", bevel=0.0015, segs=2)
    return p


def glove(side):
    main_guard()
    obj, ts = build(side)
    radii(obj, ts)
    c = cuff(obj, ts)
    bpy.ops.object.select_all(action="DESELECT")
    for o in (obj, c):
        o.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    obj.name = "glove"
    arm = rig(ts)
    obj.parent = arm
    mod = obj.modifiers.new("rig", "ARMATURE")
    mod.object = arm
    house = make(housing())
    parent(house, arm)
    report()
    export(f"assets/models/hands/glove_{side}.glb")


for s in ("right", "left"):
    glove(s)
