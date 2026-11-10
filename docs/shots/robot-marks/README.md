# Bullet marks, near and far

`near-far.jpg`: a heavy under fire (`EXTRACTION_HIT_SHOW=1`, a round into
each part every 12 frames), 1920x1080, by day, the hull and front leg
enlarged the same for both. Left at 11 m, inside `fx::MARKS_SEEN` (12 m):
its marks are drawn. Right at 13 m, past it: not drawn. A mark is a 1.4-3.5
cm disc, a pixel or two at 11 m, so the two read alike; past 12 m each was
only a draw (Marrow's radio mast drew 320 of them on one heavy). Through a
scope the reach is 12 m times its magnification (`fx::MarkReach`).

    data=$(mktemp -d); EXTRACTION_HIT_SHOW=1 EXTRACTION_WARMUP=0 WGPU_BACKEND=vulkan \
      xvfb-run -a -s "-screen 0 1920x1080x24" ./target/fast/extraction --map marrow_creek \
      --data-dir "$data" --new-game --no-robots --weather clearing --hour day \
      --camera 92.9,0,131.9,0,-2 --gallery robots:heavy+hits,20,11 \
      --screenshot heavy_11.png --frames 1000 --size 1920x1080
