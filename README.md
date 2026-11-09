# lock-input-logger

A small root daemon that records which input devices are active while your desktop session is locked.

It follows the logind `LockedHint` of one user's `seat0` session. While the session is locked, it reads every keyboard, mouse, touchpad and media-key device (including Bluetooth AVRCP devices such as headphones that connect later) and writes one line per burst of activity to the systemd journal:

```
LOCKED at 11:51:17
device added: 'MOMENTUM 4 (AVRCP)' (/dev/input/event22)
12:24:03-12:24:06 'DELL Dell QuietKey Keyboard' typing=9 repeat=0 media=0 button=0 pointer=0
UNLOCKED at 12:41:33
```

Each line gives the device name, the time range and a count per category:

| Category | Meaning |
|---|---|
| `typing` | key presses |
| `repeat` | key auto-repeat (a held or stuck key) |
| `media` | media keys (play/pause, volume, next/previous) |
| `button` | mouse, touchpad and controller buttons |
| `pointer` | pointer motion and touch movement |

Key codes are never recorded, so the log cannot reveal what was typed. While the session is unlocked, nothing is logged. Devices are read without grabbing them, so the compositor and lock screen keep working normally.

Read the log with:

```
journalctl -u lock-input-logger
```

The user to watch is set with `LOCK_LOGGER_USER` (in `/etc/conf.d/lock-input-logger` on Arch, `/etc/default/lock-input-logger` on Debian). The service exits with status 2 and stays stopped while it is unset.

## Requirements

### Runtime

- Linux with evdev input devices (`/dev/input/event*`)
- systemd, with systemd-logind and the journal
- A D-Bus system bus
- A lock screen that sets the logind `LockedHint` on the session, such as Noctalia
- Root privileges for the service, which reads `/dev/input` directly

### Build

- Rust 1.85 or newer with cargo (the crate uses edition 2024)
- Network access to crates.io for the first build

### Packaging

- Arch Linux: `base-devel` (for `makepkg`), x86_64
- Debian: `dpkg-dev` and `debhelper` (compat level 13 or newer); Debian 13 (trixie) or newer for a recent enough `rustc`
