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

If all cores read ~400000, this is the power-supply limit, not an engine bug.

Check the charger:

```sh
cat /sys/class/power_supply/ucsi-source-psy-USBC000:002/voltage_now
cat /sys/class/power_supply/ucsi-source-psy-USBC000:002/current_now
```

`5000000` (5 V) and `3000000` (3 A) means the laptop only pulls 15 W, even
if the charger is a 65 W model. Check what the charger offers:

```sh
cat /sys/class/typec/port0-partner/usb_power_delivery/source-capabilities/*fixed*/voltage
```

If that lists `20000mV` (and `maximum_current` 3250mA), the charger is fine
and the **negotiation** is stuck: try, in this order:

1. Unplug, wait 10 s, replug.
2. The other USB-C port.
3. Another USB-C cable (5 A / e-marked); a 3 A cable without an e-marker
   makes many chargers stay at 5 V.
4. EC reset: shut down, unplug the charger, hold the power button 30 s, plug
   the charger, boot.
5. Temporarily set the charge threshold to 100 % and replug
   (`/sys/class/power_supply/BAT0/charge_control_end_threshold`).
6. BIOS: USB-C / power delivery options, then BIOS/EC update.

If a known-good 65 W charger and cable still negotiate 5 V, the PD controller
or EC needs service. A reboot only helps for a few minutes on the small
supply.

**EC reset that worked on this machine:** shut down, unplug the charger, hold
the power button for 30 seconds, plug the charger, boot. Afterwards the CPU
boosted to 3.8 GHz and the battery charged again (~30 W), although the UCSI
readings still showed 5 V / 3 A - treat those as unreliable when they
contradict the battery's charge rate. The 30-second hold, not a normal
reboot, is what clears it.

Earlier this was mistaken for an `intel_pstate`/EC hang; that is the same
symptom on this machine, but the measurable cause is the negotiated 15 W.

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
