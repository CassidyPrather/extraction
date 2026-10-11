# Cloud Setup

`scripts/cloud-setup.sh` provisions a fresh Ubuntu machine for this template:
the pinned Rust toolchain with the wasm target, ALSA headers, `wasm-opt`,
Xvfb, Blender, and GIMP. It is written for the **Setup script** field of a
[Claude Code cloud environment](https://code.claude.com/docs/en/cloud-environments#setup-scripts),
and works the same on any Debian/Ubuntu box.

## Claude Code cloud environments

1. At [claude.ai/code](https://claude.ai/code), open the cloud environment menu
   in the session's title bar and choose **Edit** (or create a new
   environment).
2. Paste this into **Setup script**:

   ```bash
   #!/bin/bash
   curl -fsSL https://raw.githubusercontent.com/CassidyPrather/game-template/main/scripts/cloud-setup.sh | bash
   ```

   That fetches the script from this repo, so the environment does not
   depend on which repo a session clones. Paste the whole file instead if you
   would rather not fetch it at startup.
3. For the latest Blender and GIMP, set **Network access** to **Custom**, tick
   **Also include default list of common package managers**, and add:

   ```
   download.blender.org
   download.gimp.org
   ```

   On the default **Trusted** level those two hosts are blocked, and the
   script falls back to Ubuntu 24.04's packages (Blender 4.0, GIMP 2.10).

The script runs before Claude starts. It takes one to two minutes, well under
the ~5 minute limit for the environment to be cached, so later sessions skip
it. It always exits 0, because a failing setup script stops the session from
starting. Read the summary at the end of the setup log to see if anything
failed.

## Anywhere else

```bash
./scripts/cloud-setup.sh
```

Run it as yourself rather than with `sudo`: apt goes through `sudo`, and Rust
installs for your user. Running it again is cheap; it skips anything already
installed.

## Knobs

Set these as environment variables, either in the cloud environment's
**Environment variables** or in front of the command
(`curl ... | BLENDER_VERSION=apt bash`).

| Variable | Default | Meaning |
| - | - | - |
| `BLENDER_VERSION` | `latest` | An exact release (`5.2.2`), `apt` for Ubuntu's package, or `skip` |
| `GIMP_VERSION` | `latest` | An exact release (`3.2.6`), `apt` for Ubuntu's package, or `skip` |
| `RUST_TOOLCHAIN` | `stable` | The default toolchain. Any cloned repo with a `rust-toolchain.toml` also gets its pinned one |

Official builds go into `/opt/blender-<version>` and `/opt/gimp-<version>`,
linked as `blender` and `gimp` in `/usr/local/bin`. Downloads are checked
against the checksums published next to them. The official Blender build is
x86-64 only; on ARM the script uses Ubuntu's package.

## Using them headless

Blender runs scripts in background mode without a display:

```bash
blender --background --factory-startup --python make_sprites.py -- out/
```

Ubuntu's Blender package is built without OpenImageDenoise, so a Cycles
render there fails with `Build without OpenImageDenoiser`. Set
`scene.cycles.use_denoising = False` in the script, or use the official
build.

GIMP 3 needs `--batch-interpreter` in batch mode, and `--quit` so it exits
when the batch is done:

```bash
gimp -i --quit --batch-interpreter=python-fu-eval -b - < resize.py
gimp -i --quit --batch-interpreter=plug-in-script-fu-eval -b '(...)'
```

GIMP 2.10, the Ubuntu fallback, has no Python 3 plug-ins. Use Script-Fu
there, or the official build.

Anything that insists on a display can run under Xvfb:
`xvfb-run -a blender ...`.
