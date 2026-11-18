# The survey sets (#104): DP-2 field screen

The DP-2 field screen (`detector_bear`) held up at reading distance by
the Hollow well on Marrow Creek, the way you face. Each anomaly within
25 m is a ring as wide as it reaches; it doesn't see stones. 3200×1800
PNG, by day and by night.

Made with:

```
scripts/shot.sh OUT 96,0,6,-60,-20 40 --no-robots --hour day|night \
  --size 3200x1800 --gallery fp:detector_bear+up
```

`+up` holds a thing up to read (`dev::first_person_hands`). A dev pose's
posed thing counts as held for the detector (`anomaly::detector`).

The ring line is 2.6 mm (0.37°) at 0.4 m. The smallest ring's hole is
3.4 mm (0.49°). Both clear the 0.35° incidental floor
(`render::legibility::tests::the_field_screens_rings_clear_the_incidental_floor`).
