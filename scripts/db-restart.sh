#!/usr/bin/env bash
# Restart GLPI's database, or restart it with a non-default InnoDB setting.
#
#   scripts/db-restart.sh plain
#   scripts/db-restart.sh settings [innodb_log_buffer_size=32M ...]
#
# WHAT IT IS FOR. The InnoDB panel can already change 106 of this server's 137
# settings over SQL, and keep them across a restart through MariaDB's
# --init-file. Neither of those needs this script. The other 31 are read once
# at startup, before any SQL runs, so the only way to change one is to put it
# in a file the server reads at startup and then restart - and whether that
# file can be written at all depends on the operating system. Hence the first
# thing this does is work out which one it is on.
#
# EXIT CODES, because the caller renders them differently:
#   0  done
#   2  misuse (unknown mode, a denied setting, no service found)
#   3  not possible on this OS - the message says what to do instead
#   4  the restart or the write failed; any backup has been put back
#
# DRY_RUN=1 prints what it would do and runs nothing. The API uses it for the
# preview the link shows before you confirm.
set -uo pipefail

say() { printf '%s\n' "$*"; }

# ---------------------------------------------------------------- the OS
#
# /etc/os-release ID is the answer on Linux; NixOS also leaves /etc/NIXOS, and
# either is enough. Windows is detected from the environment rather than
# uname, because a backend built for Windows would not be running under
# MSYS - $OS is set to Windows_NT by Windows itself.
detect_os() {
  if [ "${OS:-}" = "Windows_NT" ] || [ -n "${WINDIR:-}" ]; then
    say windows; return
  fi
  case "$(uname -s 2>/dev/null)" in
    Darwin) say darwin; return ;;
    CYGWIN*|MINGW*|MSYS*) say windows; return ;;
  esac
  if [ -e /etc/NIXOS ] || { [ -r /etc/os-release ] && . /etc/os-release 2>/dev/null && [ "${ID:-}" = "nixos" ]; }; then
    say nixos; return
  fi
  say linux
}

# The unit name differs by distro and by era: mariadb.service on Debian and
# Fedora, mysql.service on NixOS and older Debian, mysqld.service on RHEL.
# Asked of systemd rather than guessed, and only units that exist count.
find_unit() {
  local u
  for u in mariadb.service mysql.service mysqld.service; do
    if systemctl list-unit-files "$u" --no-legend 2>/dev/null | grep -q .; then
      say "$u"; return 0
    fi
  done
  return 1
}

# A system unit restarted from a user service needs privilege. pkexec asks
# polkit, which means a human consents to this specific action - the property
# sudo in a NOPASSWD rule would throw away. On NixOS the setuid copy is the
# one under /run/wrappers/bin; the one in the system profile is not setuid.
escalation() {
  if [ -x /run/wrappers/bin/pkexec ]; then say /run/wrappers/bin/pkexec; return 0; fi
  if command -v pkexec >/dev/null 2>&1; then command -v pkexec; return 0; fi
  if command -v sudo >/dev/null 2>&1; then say "sudo -n"; return 0; fi
  return 1
}

# Uptime either side is the proof the restart happened. Read over the socket
# as whoever runs this; it needs no password and no privilege beyond what the
# app's own database account already has.
uptime_seconds() {
  mariadb --protocol=socket -N -B -e "SHOW GLOBAL STATUS LIKE 'Uptime'" 2>/dev/null | awk '{print $2}'
}

run() {
  if [ "${DRY_RUN:-}" = "1" ]; then
    say "would run: $*"
    return 0
  fi
  "$@"
}

# ------------------------------------------------------------- the modes

restart_plain() {
  local os="$1" unit esc before after
  case "$os" in
    nixos|linux)
      unit="$(find_unit)" || { say "No MariaDB or MySQL systemd unit found."; return 2; }
      esc="$(escalation)" || { say "No pkexec and no sudo, so a system unit cannot be restarted from here."; return 2; }
      before="$(uptime_seconds)"
      say "OS: $os   unit: $unit   escalating with: $esc"
      say "Uptime before: ${before:-unknown}s"
      say ""
      say "A restart drops every connection. GLPI wraps each inventory import in"
      say "one transaction, so an import in flight becomes a half-written asset"
      say "that still looks current - see ~/docs/gl-win-install.md."
      say ""
      # shellcheck disable=SC2086
      run $esc systemctl restart "$unit" || return 4
      [ "${DRY_RUN:-}" = "1" ] && return 0
      sleep 2
      after="$(uptime_seconds)"
      say "Uptime after: ${after:-unknown}s"
      if [ -n "$after" ] && [ "$after" -lt "${before:-999999}" ]; then
        say "Restarted."
      else
        say "The server answered, but its uptime did not go down - it may not have restarted."
        return 4
      fi
      ;;
    windows)
      say "OS: windows (not exercised - this script has never run on Windows)"
      say ""
      say "Run as Administrator:"
      say "    Restart-Service MariaDB     # or: net stop MariaDB && net start MariaDB"
      say ""
      say "Not done from here on purpose. Windows has no polkit: a service"
      say "answering loopback HTTP has no desktop session in which to show a UAC"
      say "prompt, so the only ways to do this unattended are to run this backend"
      say "as LocalSystem or to install a standing privileged helper. Both grant"
      say "more, permanently, than one consented restart."
      return 3
      ;;
    darwin)
      say "OS: macOS"
      say ""
      say "    brew services restart mariadb       # Homebrew"
      say "    sudo launchctl kickstart -k system/org.mariadb.server"
      say ""
      say "Not done from here: no polkit, and the launchd label depends on how"
      say "the server was installed."
      return 3
      ;;
    *) say "Unknown OS."; return 2 ;;
  esac
}

# Settings that cannot be undone by setting them back.
#
# innodb_page_size validates fine and then refuses to start against a datadir
# written at another size - the remedy is a dump and reload. Written into a
# file that persists, the server stays down across every restart after. The
# value is not what is wrong, so --validate-config does not catch it.
DENIED="innodb_page_size innodb_data_file_path innodb_undo_tablespaces"

restart_with_settings() {
  local os="$1"; shift
  local pairs=("$@")
  [ "${#pairs[@]}" -eq 0 ] && pairs=("innodb_log_buffer_size=32M")

  local p name
  for p in "${pairs[@]}"; do
    case "$p" in
      *=*) ;;
      *) say "Not a setting: $p (expected name=value)"; return 2 ;;
    esac
    name="${p%%=*}"
    case " $DENIED " in
      *" $name "*)
        say "Refusing $name."
        say ""
        say "It is valid as a value and still unrecoverable: the server will not"
        say "start against an existing datadir once it changes, and a config file"
        say "keeps it that way across every later restart. Changing it means a dump"
        say "and reload, not a revert."
        return 2
        ;;
    esac
  done

  say "OS: $os   settings: ${pairs[*]}"
  say ""

  case "$os" in
    nixos)
      say "Not something this script can do on NixOS - and not something you have"
      say "to do in two steps either. The rebuild below restarts the server for you."
      say ""
      say "Why the write is impossible: the server is started with"
      say "--defaults-file=/etc/my.cnf, which is a symlink into the Nix store -"
      say "mode r--r--r--, owned by root, on a read-only filesystem. Appending to"
      say "it fails as root. Its four lines are generated from services.mysql in"
      say "/etc/nixos/glpi.nix."
      say ""
      say "The whole change, in /etc/nixos/glpi.nix:"
      say ""
      say "    services.mysql.settings.mysqld = {"
      for p in "${pairs[@]}"; do say "      ${p%%=*} = \"${p#*=}\";"; done
      say "    };"
      say ""
      say "then, as root:"
      say ""
      say "    nixos-rebuild switch"
      say ""
      say "THAT IS THE RESTART TOO. Do not come back and use the other link"
      say "afterwards - it would be a second outage for nothing. The unit carries"
      say "X-Restart-Triggers naming my.cnf's store path, so a changed setting"
      say "hashes to a new path, the unit text changes with it, and the switch"
      say "restarts mysql.service as part of applying the generation. ExecStart"
      say "points at the stable /etc/my.cnf, which is exactly why the trigger has"
      say "to exist. Restarting WITHOUT a rebuild can never pick one of these up:"
      say "until the rebuild runs there is no new config to read."
      say ""
      say "The cost comes with it, unannounced: that switch restarts GLPI's"
      say "database while doing whatever else the generation changes, and a"
      say "nixos-rebuild gives no warning about in-flight inventory imports. The"
      say "link you just clicked at least says so first."
      say ""
      say "Before doing any of it, check whether the setting needs a restart at"
      say "all: 106 of this server's 137 InnoDB settings are dynamic, and the"
      say "panel can change those over SQL. Keeping one of those across a restart"
      say "needs --init-file, which is itself a one-off my.cnf line and so a"
      say "one-off rebuild - after that no root at all. Only these 31 need a file"
      say "every time:"
      say ""
      mariadb --protocol=socket -N -B -e \
        "SELECT LOWER(VARIABLE_NAME) FROM information_schema.SYSTEM_VARIABLES
         WHERE VARIABLE_NAME LIKE 'INNODB%' AND READ_ONLY='YES' ORDER BY 1" 2>/dev/null |
        tr '\n' ' ' | fold -s -w 72 | sed 's/^/    /'
      return 3
      ;;
    linux)
      local dir file unit esc backup
      for dir in /etc/mysql/mariadb.conf.d /etc/mysql/conf.d /etc/my.cnf.d; do
        [ -d "$dir" ] && break || dir=""
      done
      if [ -z "$dir" ]; then
        say "No drop-in directory found (/etc/mysql/mariadb.conf.d, /etc/my.cnf.d)."
        say "This server may read a single /etc/my.cnf; writing that is not"
        say "something this script will do blind."
        return 2
      fi
      file="$dir/99-glpi-inv.cnf"
      unit="$(find_unit)" || { say "No MariaDB or MySQL systemd unit found."; return 2; }
      esc="$(escalation)" || { say "No pkexec and no sudo."; return 2; }

      # Validated before anything is written: the server itself says whether
      # it would accept these, and it is the only authority that counts.
      local args=()
      for p in "${pairs[@]}"; do args+=("--${p//_/-}"); done
      say "Validating with the server's own parser:"
      if ! mariadbd --validate-config --datadir=/tmp "${args[@]}" 2>&1 | grep -qv '^$'; then
        say "    (no output - accepted)"
      else
        mariadbd --validate-config --datadir=/tmp "${args[@]}" 2>&1 | grep -iE "error|warning" | sed 's/^/    /' && {
          if mariadbd --validate-config --datadir=/tmp "${args[@]}" 2>&1 | grep -qiE "\[ERROR\]"; then
            say ""
            say "The server rejects these. Nothing written, nothing restarted."
            return 4
          fi
        }
      fi

      say ""
      say "Writing $file and restarting $unit with $esc."
      backup="$file.glpi-inv-backup"
      if [ -f "$file" ]; then
        # shellcheck disable=SC2086
        run $esc cp -p "$file" "$backup" || return 4
      fi
      {
        say "# Written by glpi-inv scripts/db-restart.sh on $(date -Is)."
        say "[mysqld]"
        for p in "${pairs[@]}"; do say "${p%%=*} = ${p#*=}"; done
      } > /tmp/glpi-inv-dropin.$$
      # shellcheck disable=SC2086
      run $esc install -m 0644 /tmp/glpi-inv-dropin.$$ "$file" || { rm -f /tmp/glpi-inv-dropin.$$; return 4; }
      rm -f /tmp/glpi-inv-dropin.$$
      # shellcheck disable=SC2086
      if ! run $esc systemctl restart "$unit"; then
        say ""
        say "The restart failed. Putting the config back and restarting again."
        # shellcheck disable=SC2086
        if [ -f "$backup" ]; then run $esc mv "$backup" "$file"; else run $esc rm -f "$file"; fi
        # shellcheck disable=SC2086
        run $esc systemctl restart "$unit"
        return 4
      fi
      say "Restarted with the setting in place. To undo it, delete $file and restart again."
      ;;
    windows)
      say "Not exercised - this script has never run on Windows."
      say ""
      say "The file is writable there, unlike on NixOS: MariaDB's installer puts"
      say "my.ini in the data directory (C:\\Program Files\\MariaDB 11.4\\data\\my.ini);"
      say "MySQL uses C:\\ProgramData\\MySQL\\MySQL Server 8.0\\my.ini. As"
      say "Administrator, add under [mysqld]:"
      say ""
      for p in "${pairs[@]}"; do say "    ${p%%=*} = ${p#*=}"; done
      say ""
      say "    Restart-Service MariaDB"
      say ""
      say "Validate first, with the same flag this script uses on Linux:"
      say ""
      say "    mariadbd --validate-config ${pairs[*]/#/--}"
      say ""
      say "Not done from here for the same reason as the plain restart: no polkit,"
      say "so no way to ask consent for one action."
      return 3
      ;;
    darwin)
      say "Not possible from here. Homebrew's my.cnf is at"
      say "\$(brew --prefix)/etc/my.cnf; edit it and 'brew services restart mariadb'."
      return 3
      ;;
    *) say "Unknown OS."; return 2 ;;
  esac
}

# ------------------------------------------------------------------ main

os="$(detect_os)"
mode="${1:-}"
[ $# -gt 0 ] && shift

case "$mode" in
  plain)    restart_plain "$os" ;;
  settings) restart_with_settings "$os" "$@" ;;
  detect)   say "$os" ;;
  *)
    say "usage: db-restart.sh plain | settings [name=value ...] | detect"
    exit 2
    ;;
esac
