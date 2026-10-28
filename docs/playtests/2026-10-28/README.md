# Playtest, 28 October (night)

The designer's own session on build 2798e450, launched with `--mirror`
(without it, the window had no camera once the headset session began and
the GPU hung; being fixed). Windows 10, i7-8700K, RTX 3080, SteamVR with a
Bigscreen Beyond 2 (2980x2980 per eye recommended) and Index controllers.

Verdict, in the designer's words: focus on polish, depth and QA over new
features, onboarding above all. "The game definitely has the sauce";
what's left is polishing, cutting the jank and being more conscientious
about the design. They couldn't make sense of the playtest mark. A spoken
transcript follows.

- `logs/session-1793168679.log`: the Training Yard (onboarding), and its
  frame log `logs/frames-1793168680-training_yard.txt`.
- `logs/session-1793170661.log`: Marrow Creek, and
  `logs/frames-1793170661-marrow_creek.txt`.
- `raids/`: the raids' telemetry (`raid001` the long one).
- `logs/session-17931684*.log`, `logs/crash-*.txt`: the short runs before
  `--mirror`, and the swapchain teardown panic.
- `settings.ron`: the settings as played.

Frame logs are one line a frame, `frame cpu_ms gpu_ms eye_scale`
(`render::budget`). The username in paths is replaced by `<user>`.
