# The Gate's lamp while it counts

`gate_lamp_60m.jpg`: Marrow's Gate from 60 m on the lock side, quiet |
counting, by day and night, clear and in a downpour. Each counting frame is
held at its dimmest (`EXTRACTION_GATE_SHOW=trough`), the worst moment of the
swell. The lamp is a red band across each face of the lintel
(`render::gate_lamp`). It wakes at full strength the frame a key crosses the
60 m line (`heavy_mind::GateCount`). It swells from 0.75 Hz up to 2 Hz as the
count runs out, never below 45% and never as a blink. After dark it carries
a halo held to an angle. It fades out within 0.4 s once the Gate goes quiet.

The measure is `tells_range`'s: pixels changed by more than 12 levels,
counting against quiet, at the 1080-line view's scale. The bar is a 0.4°
square (24 px):

| | 60 m | 70 m |
|---|---|---|
| day, clear | 327 (x30) | 232 (x22) |
| day, downpour | 328 (x31) | 225 (x21) |
| night, clear | 331 (x31) | 234 (x22) |
| night, downpour | 343 (x32) | 245 (x23) |

Every change falls inside the band's own box. Nothing round it is lit.
