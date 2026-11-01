#!/usr/bin/env python3
"""A camera pose for a placed prefab, for --camera (docs/craft/PROP_READING.md).

    scripts/prop_pose.py AT_X AT_Z ROT LX LZ [TX TZ [PITCH]]

Stands at the prefab's local (LX, LZ) (its front is -Z), looking at its
local (TX, TZ) (default its middle), the prefab placed at (AT_X, AT_Z)
turned ROT degrees as `buildings::placement_transform` turns it. Prints
x,0,z,yaw,pitch (y 0: eyes 1.7 m over the ground).
"""
import math
import sys

ax, az, rot, lx, lz = map(float, sys.argv[1:6])
tx, tz = (float(sys.argv[6]), float(sys.argv[7])) if len(sys.argv) > 7 else (0.0, 0.0)
pitch = float(sys.argv[8]) if len(sys.argv) > 8 else -4.0
phi = -math.radians(rot)


def world(x, z):
    return (ax + x * math.cos(phi) + z * math.sin(phi), az - x * math.sin(phi) + z * math.cos(phi))


cx, cz = world(lx, lz)
px, pz = world(tx, tz)
# --camera's yaw turns clockwise from -Z.
yaw = math.degrees(math.atan2(px - cx, -(pz - cz)))
print(f"{cx:.2f},0,{cz:.2f},{yaw:.1f},{pitch}")
