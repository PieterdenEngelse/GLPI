# GLPI 12.0.1 - IT asset management and helpdesk. Loopback only.
#
# nixpkgs has neither a `glpi` package nor a `services.glpi` module - checked
# with builtins.hasAttr against this machine's own pkgs and options, not by
# guessing from `nix search` suggestions - so both are defined here.
#
# The whole design rests on one upstream detail. GLPI normally writes inside
# its own source tree, which /nix/store forbids. But
# src/Glpi/Application/SystemConfigurator.php defines every path constant as:
#
#   if (!defined($name) && ($value = getenv($name)) !== false) define($name, $value);
#
# so each one can be pointed at /var/lib/glpi instead. Setting GLPI_VAR_DIR is
# enough for most of them: _cache, _log, _sessions, _tmp and the rest default
# to '{GLPI_VAR_DIR}/...' and GLPI creates them itself on first run. Only
# config, files and marketplace need naming explicitly.
#
# Install is NOT automatic, and deliberately so: creating the schema is a
# one-time action with a destructive --force variant, not something a rebuild
# should ever do on its own. After the first rebuild, run EXACTLY this:
#
#   sudo -u glpi glpi-console database:install \
#     --db-name=glpi --db-user=glpi --db-password= --no-interaction
#
# `--db-password=` with an EMPTY VALUE is required and is not a typo. The
# account uses unix_socket auth and has no password (see services.mysql
# below), but GLPI validates the option itself and rejects a NULL value:
# omitting it fails with "--db-password option value cannot be null." and
# --no-interaction means it cannot prompt for one. An empty string passes
# validation, and MariaDB then ignores it entirely because authentication is
# by OS user - which `sudo -u glpi` satisfies.
#
# --no-interaction is worth keeping for a reason unrelated to prompts: it
# makes an accidental RE-run refuse instead of acting. Both of GLPI's
# confirmation prompts default to Yes, including "Do you want to reconfigure
# database?", which overwrites a working install. Non-interactively that
# branch is skipped and execute() returns ERROR_DB_CONFIG_ALREADY_SET,
# so clobbering requires typing --reconfigure or --force on purpose.
#
# --no-telemetry is NOT needed: install/empty_data.php registers the
# telemetry crontask with state => CronTask::STATE_DISABLE, so a fresh
# install has it off already. Change it later under
# Setup > Automatic actions > telemetry; it is just that task's state column.
#
# Then log in at http://localhost:8080 with glpi/glpi and CHANGE THAT
# PASSWORD - GLPI ships four accounts with published defaults (glpi, tech,
# normal, post-only). Disable or delete the ones you do not need.
{ config, lib, pkgs, inputs, ... }:

let
  cfg = config.services.glpi;

  # PHP comes from the `nixpkgs-php` input, NOT from `pkgs`. The system's main
  # nixpkgs pin carries php 8.4.26, which is vulnerable to CVE-2026-109618
  # (CVSS 9.8, RCE) and CVE-2026-109619 (CVSS 9.2, RCE, actively exploited and
  # in CISA's KEV catalog). Both are interpreter bugs, so no GLPI version can
  # avoid them - only a newer PHP can. See the comment on the input in
  # flake.nix for why the whole system was not moved forward instead.
  #
  # Everything else here - nginx, mariadb - still comes from `pkgs`. Mixing two
  # nixpkgs revisions in one closure is fine; they simply do not interact.
  phpPkgs = inputs.nixpkgs-php.legacyPackages.${pkgs.stdenv.hostPlatform.system};

  glpi = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
    pname = "glpi";
    version = "12.0.1";

    # The RELEASE TARBALL, not a git checkout: it ships vendor/ already
    # populated. A git source would need `composer install`, which cannot run
    # in a sandboxed build without vendoring the entire dependency tree first.
    src = pkgs.fetchurl {
      url = "https://github.com/glpi-project/glpi/releases/download/${finalAttrs.version}/glpi-${finalAttrs.version}.tgz";
      hash = "sha256-v4mhvkDqduTnpqXOqP0kydgi+rg+TpBRn/DI0GLxMpE=";
    };

    dontBuild = true;
    dontFixup = true;

    installPhase = ''
      runHook preInstall
      mkdir -p $out
      cp -r . $out/
      runHook postInstall
    '';

    meta = {
      description = "IT asset management, issue tracking and service desk";
      homepage = "https://glpi-project.org/";
      license = lib.licenses.gpl3Plus;
      platforms = lib.platforms.linux;
    };
  });

  # php84 pinned explicitly rather than `php`: GLPI 12 requires >= 8.3 and
  # GLPI targets the 8.3/8.4 lines. 8.5 would be ahead of what upstream tests.
  #
  # `enabled ++` and not a bare list - dom, filter, libxml, simplexml,
  # tokenizer, session, openssl, zlib, ctype, hash, iconv and json are already
  # in nixpkgs' default set, and replacing it wholesale would silently drop the
  # ones GLPI needs.
  php = phpPkgs.php84.buildEnv {
    extensions =
      { all, enabled }:
      enabled
      ++ (with all; [
        bcmath
        curl
        gd
        intl
        mbstring
        mysqli
        opcache
        zip # plugin and marketplace archives
        bz2 # also marketplace: GLPI's own requirements check reports
        # "Following extensions are not present: bz2" without it
        exif # image metadata on uploaded documents
        ldap # directory authentication; harmless if unused
        apcu # GLPI's preferred local cache backend
      ]);

    extraConfig = ''
      memory_limit = 256M
      upload_max_filesize = 100M
      post_max_size = 100M
      max_execution_time = 600

      ; Comments here MUST use ';' - php.ini dropped '#' in PHP 7, so a '#'
      ; line is parsed as a setting and any quote in it is a syntax error.
      ;
      ; mysqli resolves 'localhost' to a socket, and NixOS does not put
      ; MariaDB's where PHP looks by default. Without this GLPI fails to
      ; reach the database at install time.
      mysqli.default_socket = /run/mysqld/mysqld.sock

      ; GLPI's status page warns loudly if PHP has no timezone.
      date.timezone = "${config.time.timeZone}"

      session.cookie_httponly = 1
      session.cookie_samesite = "Lax"
    '';
  };

  # Handed to php-fpm, to the cron unit and to the console wrapper. All three
  # must agree: a console run with different paths would write a config_db.php
  # the web process cannot see.
  glpiEnv = {
    GLPI_CONFIG_DIR = "${cfg.stateDir}/config";
    GLPI_VAR_DIR = "${cfg.stateDir}/files";
    GLPI_MARKETPLACE_DIR = "${cfg.stateDir}/marketplace";
  };

  # bin/console with the environment already set, so routine administration is
  # `sudo -u glpi glpi-console <cmd>` instead of a line of exports.
  glpi-console = pkgs.writeShellScriptBin "glpi-console" ''
    ${lib.concatStringsSep "\n" (lib.mapAttrsToList (n: v: "export ${n}=${lib.escapeShellArg v}") glpiEnv)}
    exec ${php}/bin/php ${cfg.package}/bin/console "$@"
  '';
in
{
  options.services.glpi = {
    enable = lib.mkEnableOption "GLPI asset management and service desk";

    package = lib.mkOption {
      type = lib.types.package;
      default = glpi;
      defaultText = lib.literalExpression "the GLPI 12.0.1 release tarball, built in this file";
      description = ''
        The GLPI source tree. Exposed so it can be built and inspected on its
        own (`nix build .#nixosConfigurations.nixos.config.services.glpi.package`)
        rather than only as part of the system closure.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 8080;
      description = "Port nginx serves GLPI on, bound to 127.0.0.1 only.";
    };

    hostname = lib.mkOption {
      type = lib.types.str;
      default = "${config.networking.hostName}.local";
      defaultText = lib.literalExpression ''"\${config.networking.hostName}.local"'';
      description = ''
        The name clients use to reach this server, and the name the TLS
        certificate is issued for. A `.local` name resolves over mDNS, which
        needs services.avahi.publish enabled - avahi only ANSWERS for other
        hosts' names by default, it does not announce its own.
      '';
    };

    lan = {
      enable = lib.mkEnableOption ''
        serving GLPI to the local network over HTTPS, in addition to the
        plain-HTTP loopback listener. Opens the firewall port
      '';

      port = lib.mkOption {
        type = lib.types.port;
        default = 443;
        description = "HTTPS port for LAN access.";
      };

      tlsDir = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/glpi-tls";
        description = ''
          Where the self-signed certificate and its key are generated.
          Deliberately NOT in the Nix store: everything there is
          world-readable, which is fine for the certificate and unacceptable
          for the private key.
        '';
      };
    };

    stateDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/glpi";
      description = ''
        Writable root for config, files and marketplace. The GLPI source stays
        read-only in the store; everything mutable lives here.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.glpi = {
      isSystemUser = true;
      group = "glpi";
      home = cfg.stateDir;
    };
    users.groups.glpi = { };

    # 0750: the config dir holds config_db.php with the database credentials
    # and the encryption key, so it must not be world-readable.
    systemd.tmpfiles.rules = [
      "d ${cfg.stateDir} 0750 glpi glpi -"
      "d ${cfg.stateDir}/config 0750 glpi glpi -"
      "d ${cfg.stateDir}/files 0750 glpi glpi -"
      "d ${cfg.stateDir}/marketplace 0750 glpi glpi -"
    ];

    # No password anywhere: ensureUsers creates the account with MariaDB's
    # unix_socket plugin, so the glpi OS user authenticates by being that user
    # over the local socket. That is why `database:install` below is given no
    # --db-password.
    services.mysql = {
      enable = true;
      package = pkgs.mariadb;
      ensureDatabases = [ "glpi" ];
      ensureUsers = [
        {
          name = "glpi";
          ensurePermissions = {
            "glpi.*" = "ALL PRIVILEGES";
          };
        }
      ];
    };

    services.phpfpm.pools.glpi = {
      user = "glpi";
      group = "glpi";
      phpPackage = php;
      phpEnv = glpiEnv;
      settings = {
        "listen.owner" = config.services.nginx.user;
        "listen.group" = config.services.nginx.group;
        "pm" = "dynamic";
        "pm.max_children" = 32;
        "pm.start_servers" = 2;
        "pm.min_spare_servers" = 2;
        "pm.max_spare_servers" = 4;
        # Without this, a PHP fatal goes only to GLPI's own log and the journal
        # shows a bare 502.
        "catch_workers_output" = true;
      };
    };

    # The certificate is generated AT RUNTIME, not in a derivation. Anything
    # built by Nix lands in /nix/store, which is world-readable - fine for the
    # certificate, fatal for the private key. So a oneshot unit creates both
    # under lan.tlsDir on first boot and leaves them alone afterwards.
    #
    # before = nginx: nginx refuses to start when ssl_certificate points at a
    # file that does not exist, so on the very first boot the ordering is the
    # difference between a working server and a failed unit.
    systemd.services.glpi-cert = lib.mkIf cfg.lan.enable {
      description = "Generate GLPI's self-signed TLS certificate";
      wantedBy = [ "multi-user.target" ];
      before = [ "nginx.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      script = ''
        set -eu
        dir=${cfg.lan.tlsDir}
        install -d -m 0755 "$dir"

        # Generate only when missing - regenerating on every boot would
        # invalidate the copy already distributed to every client.
        if [ -f "$dir/cert.pem" ] && [ -f "$dir/key.pem" ]; then
          echo "certificate already present, leaving it alone"
        else

        # 10 years: this is a self-signed cert for a home LAN, and an expiry
        # would break every agent at once with an error that looks nothing
        # like "your certificate ran out".
        #
        # subjectAltName is NOT optional. Browsers and the agent's OpenSSL
        # both ignore CN entirely and match on SAN only, so a cert with just
        # a CN is rejected by everything that reads it.
          ${pkgs.openssl}/bin/openssl req -x509 -newkey rsa:4096 -sha256 \
            -days 3650 -nodes \
            -keyout "$dir/key.pem" -out "$dir/cert.pem" \
            -subj "/CN=${cfg.hostname}" \
            -addext "subjectAltName=DNS:${cfg.hostname},DNS:${config.networking.hostName}" \
            -addext "basicConstraints=critical,CA:TRUE" \
            -addext "keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign"
        fi

        # Enforced on EVERY run, deliberately outside the branch above.
        # An install that already has a key from a previous version of
        # this unit must have its mode corrected too, and a oneshot that
        # only fixed permissions it had just created would leave such a
        # key unreadable by nginx forever.

        # cert.pem is handed to every client as its trust anchor, so it is
        # readable by all.
        chmod 0644 "$dir/cert.pem"

        # key.pem is 0640 root:nginx, NOT 0600 root. The obvious reasoning -
        # "nginx's master runs as root, so root-only is enough" - is wrong on
        # NixOS: the generated nginx.service sets User=nginx for the WHOLE
        # unit, including the ExecStartPre that runs `nginx -t`. That test
        # opens the key as the nginx user and fails with
        #
        #   nginx: [emerg] cannot load certificate key ".../key.pem":
        #   BIO_new_file() failed (SSL: ... Permission denied ...)
        #
        # which aborts the start; after a few restarts systemd gives up with
        # start-limit-hit. Group-read for nginx and nothing wider is the fix.
        # The directory stays 0755 so nginx can traverse it.
        chown root:${config.services.nginx.group} "$dir/key.pem"
        chmod 0640 "$dir/key.pem"
      '';
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.lan.enable [ cfg.lan.port ];

    services.nginx = {
      enable = true;
      recommendedGzipSettings = true;
      recommendedOptimisation = true;
      recommendedTlsSettings = true;
      virtualHosts."glpi" = {
        # ONE vhost with two listeners, not two vhosts: the locations below
        # are the routing rules for GLPI and must not be allowed to drift
        # between a local and a LAN copy.
        #
        # The loopback listener stays plain HTTP and is always present - it is
        # how this machine itself reaches GLPI, and 127.0.0.1 cannot be
        # reached from the network whatever the firewall says. The TLS
        # listener is the one that needs both an open port and a certificate.
        listen = [
          {
            addr = "127.0.0.1";
            port = cfg.port;
          }
        ]
        ++ lib.optional cfg.lan.enable {
          addr = "0.0.0.0";
          port = cfg.lan.port;
          ssl = true;
        };

        # addSSL, even though the listen entry above already says ssl = true.
        # Setting the listener is NOT enough: nginx/default.nix derives
        #   hasSSL = onlySSL || addSSL || forceSSL
        # and emits ssl_certificate only under that flag, so without this the
        # generated config has `listen 0.0.0.0:443 ssl` and no certificate at
        # all - nginx then refuses to start. Found by grepping the generated
        # nginx.conf for ssl_certificate and getting nothing back.
        #
        # addSSL and not forceSSL: forceSSL filters the listen list down to
        # the TLS entries (hostListen, same file) and would silently delete
        # the loopback HTTP listener, plus redirect it to a hostname that
        # 127.0.0.1 is not. An explicit `listen` list is otherwise passed
        # through verbatim, so both entries survive.
        addSSL = cfg.lan.enable;

        # Paths, not store contents: these files are generated at runtime by
        # glpi-cert.service above. Referring to them by path is what keeps the
        # private key out of the world-readable store.
        sslCertificate = lib.mkIf cfg.lan.enable "${cfg.lan.tlsDir}/cert.pem";
        sslCertificateKey = lib.mkIf cfg.lan.enable "${cfg.lan.tlsDir}/key.pem";

        root = "${cfg.package}/public";

        locations."/" = {
          tryFiles = "$uri /index.php$is_args$args";
          index = "index.php";
        };

        # priority MATTERS here. nginx tries regex locations in the order they
        # appear in the generated file, taking the FIRST match - and without
        # explicit priorities the module emitted `~ \.php$` (the 404 below)
        # BEFORE this block, so every request to /index.php returned 404 and
        # GLPI never ran at all. Caught by reading the generated nginx.conf,
        # not by any build or eval error. Lower number = emitted earlier.
        locations."~ ^/index\\.php$".priority = 500;
        locations."~ ^/index\\.php$".extraConfig = ''
          fastcgi_pass unix:${config.services.phpfpm.pools.glpi.socket};
          fastcgi_split_path_info ^(.+\.php)(/.*)$;
          include ${pkgs.nginx}/conf/fastcgi_params;
          fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
          fastcgi_param DOCUMENT_ROOT $document_root;
          fastcgi_param PATH_INFO $fastcgi_path_info;
        '';

        # There is deliberately NO `location ~ \.php$ { return 404; }` here.
        #
        # It looks like sensible defence-in-depth and it breaks GLPI outright.
        # GLPI 12 still addresses its legacy entry points by .php URL - the
        # login form posts to /front/login.php - but those are ROUTES handled
        # by public/index.php, not files on disk. nginx tries regex locations
        # before the `location /` prefix, so such a block matches first and
        # returns 404 before try_files can reach the front controller. The
        # symptom is a login page that renders perfectly and then 404s the
        # moment you submit it.
        #
        # Nothing is lost by its absence: the fastcgi block above matches
        # ONLY the exact path /index.php, and index.php is the sole .php file
        # in public/, so no other file can reach the interpreter.
      };
    };

    # GLPI's scheduler. front/cron.php rather than a bin/console command
    # because GLPI 12 ships no cron console command - checked, the only thing
    # under src/Glpi/Console/Task/ is task:unlock.
    systemd.services.glpi-cron = {
      description = "GLPI task scheduler";
      after = [ "mysql.service" ];
      environment = glpiEnv;
      serviceConfig = {
        Type = "oneshot";
        User = "glpi";
        Group = "glpi";
        ExecStart = "${php}/bin/php ${cfg.package}/front/cron.php";
      };
    };

    systemd.timers.glpi-cron = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = "minutely";
        # A laptop is asleep most of the day; without this, notifications and
        # scheduled tasks simply never fire for those hours.
        Persistent = true;
        RandomizedDelaySec = "15s";
      };
    };

    environment.systemPackages = [ glpi-console ];
  };
}
