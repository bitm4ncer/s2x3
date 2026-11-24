# Troubleshooting

## The whole machine is slow: CPU locked at 400 MHz

Symptom: every game stutters, not just Slope3 or one engine. All cores stay
at ~400 MHz under full load, while the governor and EPP report
`performance`, the package stays cool (~46 °C) and the fan spins (~2450 RPM).

Check:

```sh
for i in $(seq 16); do yes >/dev/null & done
sleep 3
for c in 0 4 8; do cat /sys/devices/system/cpu/cpu$c/cpufreq/scaling_cur_freq; done
pkill -x yes
```

If all cores read ~400000, this is the known ThinkPad/EC power-state hang on
this machine (seen with OpenTricky before), not an engine bug.

Fix: **reboot**. After the reboot the same test shows ~3.8 GHz.

A short S3 cycle only helps the separate EC fan-table bug (`speed=65535`,
handled by `/usr/local/bin/fan-reset.sh`); it does not clear this lock.

## Depth readback stalls

Log lines `[gfx] depth of frame N: the readback never completed` mean the
light query waits 60 ms per frame on this GPU/driver. `SSX3_DEPTH_SYNC=0`
removes them; the proper fix belongs upstream (see
`docs/upstream/2026-11-24-depth-readback.md`).

## Frame rate vs resolution

On the Iris Xe in this laptop, with the CPU healthy: 640x480 ≈ 60 fps,
720p ≈ 52, 1080p ≈ 39. Pick the internal size in the launcher settings.
