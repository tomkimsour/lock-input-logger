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

The user to watch is set with `LOCK_LOGGER_USER`: the `services.lock-input-logger.user` option on NixOS, `/etc/default/lock-input-logger` on Debian. The service exits with status 2 and stays stopped while it is unset.

## Requirements

### Runtime

- Linux with evdev input devices (`/dev/input/event*`)
- systemd, with systemd-logind and the journal
- A D-Bus system bus
- A lock screen that sets the logind `LockedHint` on the session, such as Noctalia
- Root privileges for the service, which reads `/dev/input` directly

### Build

- Rust 1.89 or newer with cargo
- Network access to crates.io for the first build

### Packaging

- NixOS: Nix with flakes enabled
- Debian: `dpkg-dev` and `debhelper` (compat level 13 or newer), plus Rust 1.89 or newer from [rustup](https://rustup.rs), since Debian 13's `rustc` is too old

## Installation

### NixOS

Add the flake as an input and enable the module:

```nix
{
  inputs.lock-input-logger.url = "github:tomkimsour/lock-input-logger";

  outputs = { nixpkgs, lock-input-logger, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        lock-input-logger.nixosModules.default
        {
          services.lock-input-logger = {
            enable = true;
            user = "your-login";
          };
        }
      ];
    };
  };
}
```

Apply with `sudo nixos-rebuild switch`. Removing the module (or setting `enable = false`) and rebuilding removes the service.

| Option | Description |
|---|---|
| `services.lock-input-logger.enable` | Run the logger as a hardened root service |
| `services.lock-input-logger.user` | Login name whose `seat0` session is watched (required) |
| `services.lock-input-logger.package` | Package to use; defaults to this flake's package |

The package alone is available as `lock-input-logger.packages.${system}.default`.

### Debian and Ubuntu

Download the `.deb` from the [latest release](https://github.com/tomkimsour/lock-input-logger/releases/latest) and install it:

```
sudo apt install ./lock-input-logger_*_amd64.deb
```

The service is enabled but only starts once the user is set:

```
sudo sed -i 's/^LOCK_LOGGER_USER=.*/LOCK_LOGGER_USER=your-login/' /etc/default/lock-input-logger
sudo systemctl start lock-input-logger
```

Uninstall with `sudo apt remove lock-input-logger`, which stops the service. `sudo apt purge lock-input-logger` also removes its configuration.

### From source

On any other systemd distribution, build and install with make, run as your normal user (it calls `sudo` for the install steps):

```
make install
```

This builds the release binary, installs it to `/usr/local/bin`, installs the hardened unit to `/etc/systemd/system`, and enables and (re)starts the service watching your own login. Run it again after pulling changes to update. Remove everything with:

```
make uninstall
```

| Variable | Default | Meaning |
|---|---|---|
| `LOCK_LOGGER_USER` | `$SUDO_USER`, else `$USER` | Login whose session is watched |
| `PREFIX` / `BINDIR` | `/usr/local` / `$(PREFIX)/bin` | Where the binary goes |
| `UNITDIR` | `/etc/systemd/system` | Where the unit goes |

Example: `make install LOCK_LOGGER_USER=alice`.

## Releasing

1. Bump the version in `Cargo.toml` and add a `debian/changelog` entry with the same version, then commit. The Nix package reads its version from `Cargo.toml`.
2. Tag and push: `git tag v<version> && git push origin v<version>`.

The `Release` workflow checks that both versions match the tag, runs clippy and the tests, builds the `.deb`, runs `nix flake check` (package build and a NixOS VM test of the module), and publishes a GitHub release with the `.deb`.

## Development

```
cargo test
nix flake check
```

`nix flake check` builds the package and runs a NixOS VM test that locks a session through logind, presses keys and checks the logged bursts. It needs KVM.
