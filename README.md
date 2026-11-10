# lock-input-logger

A small root daemon that records which input devices are active while your desktop session is locked.

It follows the logind `LockedHint` of one user's `seat0` session. While the session is locked, it reads every keyboard, mouse, touchpad and media-key device. This includes Bluetooth AVRCP devices, such as headphones that connect later. For each burst of activity, it writes one line to the systemd journal:

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

The logger never records key codes, so the log cannot reveal what was typed. While the session is unlocked, it logs nothing. It reads the devices without grabbing them, so the compositor and the lock screen keep working normally.

Read the log with:

```
journalctl -u lock-input-logger
```

`LOCK_LOGGER_USER` sets the user to watch. On NixOS, set it with the `services.lock-input-logger.user` option. On Debian, set it in `/etc/default/lock-input-logger`. While it is unset, the service exits with status 2 and stays stopped.

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
- Debian: `dpkg-dev` and `debhelper` (compat level 13 or newer). You also need Rust 1.89 or newer from [rustup](https://rustup.rs), because the `rustc` in Debian 13 is too old.

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

To apply the change, run `sudo nixos-rebuild switch`. To remove the service, remove the module (or set `enable = false`) and rebuild.

| Option | Description |
|---|---|
| `services.lock-input-logger.enable` | Run the logger as a hardened root service |
| `services.lock-input-logger.user` | Login name whose `seat0` session is watched (required) |
| `services.lock-input-logger.package` | Package to use (default: this flake's package) |

The package alone is available as `lock-input-logger.packages.${system}.default`.

### Debian and Ubuntu

Download the `.deb` from the [latest release](https://github.com/tomkimsour/lock-input-logger/releases/latest) and install it:

```
sudo apt install ./lock-input-logger_*_amd64.deb
```

The package enables the service, but the service starts only after you set the user:

```
sudo sed -i 's/^LOCK_LOGGER_USER=.*/LOCK_LOGGER_USER=your-login/' /etc/default/lock-input-logger
sudo systemctl start lock-input-logger
```

To uninstall, run `sudo apt remove lock-input-logger`. This also stops the service. `sudo apt purge lock-input-logger` also removes the configuration.

### From source

On any other systemd distribution, build and install with make. Run it as your normal user. It calls `sudo` for the install steps.

```
make install
```

This command:

1. Builds the release binary.
2. Installs the binary to `/usr/local/bin`.
3. Installs the hardened unit to `/etc/systemd/system`.
4. Enables and (re)starts the service, which watches your own login.

To update after you pull changes, run `make install` again. To remove everything, run:

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

1. Bump the version in `Cargo.toml`. The Nix package reads its version from this file.
2. Add a `debian/changelog` entry with the same version.
3. Commit the changes.
4. Tag and push: `git tag v<version> && git push origin v<version>`.

The `Release` workflow then:

1. Checks that the versions in `Cargo.toml` and `debian/changelog` match the tag.
2. Runs clippy and the tests.
3. Builds the `.deb`.
4. Runs `nix flake check` (package build and a NixOS VM test of the module).
5. Publishes a GitHub release with the `.deb`.

## Development

```
cargo test
nix flake check
```

`nix flake check` builds the package and runs a NixOS VM test. The test locks a session through logind, presses keys and checks the logged bursts. It needs KVM.
