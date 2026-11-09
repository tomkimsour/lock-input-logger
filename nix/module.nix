self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.lock-input-logger;
in
{
  options.services.lock-input-logger = {
    enable = lib.mkEnableOption "logging of input device activity while the session is locked";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "lock-input-logger.packages.\${system}.default";
      description = "Package providing the lock-input-logger binary.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      example = "alice";
      description = "Login name whose seat0 session is watched for lock and unlock.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.lock-input-logger = {
      description = "Log which input devices are active while the session is locked";
      after = [
        "systemd-logind.service"
        "dbus.service"
      ];
      wantedBy = [ "multi-user.target" ];
      environment.LOCK_LOGGER_USER = cfg.user;
      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;
        RestartPreventExitStatus = 2;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateNetwork = true;
        RestrictAddressFamilies = "AF_UNIX";
        DevicePolicy = "closed";
        DeviceAllow = "char-input r";
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        CapabilityBoundingSet = "";
      };
    };
  };
}
