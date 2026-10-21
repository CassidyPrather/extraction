"""26 mm signal cartridges for the SPSh-44 flare pistol: one loose cartridge
per colour. Writes assets/models/items/flare_{white,red,green}.glb (item
space: see _kit.py).

Each lies along Z, the wad (the end the star leaves by) to -Z, in a box of
0.0296 x 0.0296 x 0.078 m: a brass head with its rim and primer, a card
case of kraft card, the top closed by a wad recessed inside the rolled mouth. The colour
is painted on the wad and in a band round the top of the case, and the wad
carries raised marks to tell them apart by touch in the dark: white is
smooth, red has one bump, green two.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import centred, item, wanted
from common import Part

LENGTH = 0.078
R_RIM, R_HEAD, R_CASE = 0.0148, 0.0136, 0.0132
R_WAD = 0.0106
WAD_Z = -0.0745          # the wad's face, 3.5 mm down inside the mouth
BAND = (-0.058, -0.068)  # the painted band round the case


def flare(colour, bumps):
    def build():
        p = Part("body")
        segs = 32
        # Brass head: the rim, the extractor groove, the head swaged over
        # the case.
        p.lathe([(0.0, 0.0), (R_RIM - 0.0004, 0.0), (R_RIM, -0.0004), (R_RIM, -0.0016),
                 (R_HEAD + 0.0001, -0.0019), (R_HEAD, -0.0026), (R_HEAD, -0.0212),
                 (R_CASE + 0.0002, -0.022), (0.0, -0.022)], "Brass", segs=segs)
        # Primer.
        p.cyl(0.0032, -0.0004, 0.00003, "Copper", segs=14)
        # The card case, the colour band, and the rolled mouth round the wad.
        p.lathe([(0.0, -0.021), (R_CASE, -0.021), (R_CASE, BAND[0]), (0.0, BAND[0])], "Rope",
                segs=segs)
        p.lathe([(0.0, BAND[0]), (R_CASE, BAND[0]), (R_CASE, BAND[1]), (0.0, BAND[1])], colour,
                segs=segs)
        p.lathe([(R_WAD, BAND[1]), (R_CASE, BAND[1]), (R_CASE, -0.0765),
                 (R_CASE - 0.0006, -LENGTH), (R_WAD + 0.0007, -LENGTH), (R_WAD, -0.0768)],
                "Rope", segs=segs, closed=True)
        # The painted wad, and its marks.
        p.lathe([(0.0, BAND[1]), (R_WAD, BAND[1]), (R_WAD, WAD_Z), (0.0, WAD_Z)], colour,
                segs=segs)
        for k in range(bumps):
            x = 0.0 if bumps == 1 else (k - (bumps - 1) / 2) * 0.0085
            p.sphere(0.0022, (x, 0.0, WAD_Z + 0.0004), colour, segs=12, rings=6)
        return centred([p])
    return build


FLARES = {
    "flare_white": flare("Enamel", 0),
    "flare_red": flare("Red", 1),
    "flare_green": flare("Leaf", 2),
}

if __name__ == "__main__":
    for item_id, fn in FLARES.items():
        if wanted(item_id):
            item(item_id, fn)
