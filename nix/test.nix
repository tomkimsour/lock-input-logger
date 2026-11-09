{ pkgs, module }:
pkgs.testers.runNixOSTest {
  name = "lock-input-logger";

  nodes.machine = {
    imports = [ module ];
    users.users.alice.isNormalUser = true;
    services.getty.autologinUser = "alice";
    services.lock-input-logger = {
      enable = true;
      user = "alice";
    };
  };

  testScript = ''
    journal = "journalctl -u lock-input-logger.service -o cat"

    def set_locked(value):
        sid = machine.succeed(
            "loginctl list-sessions --no-legend | awk '$3 == \"alice\" && $4 == \"seat0\" {print $1}'"
        ).strip()
        path = machine.succeed(
            f"busctl call org.freedesktop.login1 /org/freedesktop/login1 org.freedesktop.login1.Manager GetSession s {sid}"
        ).split()[1].strip('"')
        machine.succeed(
            f"busctl call org.freedesktop.login1 {path} org.freedesktop.login1.Session SetLockedHint b {value}"
        )

    machine.wait_for_unit("lock-input-logger.service")

    with subtest("should watch the user's seat0 session"):
        machine.wait_until_succeeds(f"{journal} | grep -q 'watching logind session .* of alice'")

    with subtest("should open input devices under the hardened unit"):
        machine.wait_until_succeeds(f"{journal} | grep -q '^device added: '")

    with subtest("should log only keys pressed while locked"):
        machine.send_key("b")
        set_locked("true")
        machine.wait_until_succeeds(f"{journal} | grep -q '^LOCKED at '")
        machine.send_key("a")
        machine.wait_until_succeeds(f"{journal} | grep -q 'typing=1 repeat=0 media=0'")
        set_locked("false")
        machine.wait_until_succeeds(f"{journal} | grep -q '^UNLOCKED at '")
        machine.fail(f"{journal} | grep -q 'typing=2'")
  '';
}
