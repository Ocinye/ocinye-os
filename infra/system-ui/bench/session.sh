#!/bin/sh
# Graphics bench: a whole seat inside the guest (ADR-0031).
#
# Starts the stand-in Core, the real System Service and Session Service, and
# the compositor with the system interface as the unprivileged `ocinye-ui`
# account — the same arrangement the image's units will make. Nothing here
# ships.
#
#   session.sh DIR [IDLE_LOCK_SECS]
set -eu
dir=$1
idle=${2:-300}

id ocinye-ui >/dev/null 2>&1 || useradd --system --create-home --shell /usr/sbin/nologin --groups video,input,render ocinye-ui
install -d -m 0755 /run/ocinye /etc/ocinye /var/log/ocinye /opt/sx
install -d -m 0700 /var/lib/ocinye/session
install -d -m 0700 -o ocinye-ui -g ocinye-ui /run/sx
install -m 0755 "$dir"/ocinye-* /opt/sx/
# A stand-in for the Workspace: one local page, so that the window the
# interface opens has something to show below the bar.
if [ -d "$dir/workspace" ]; then
    printf 'http://127.0.0.1:8090/\n' > /etc/ocinye/workspace-url
    python3 -m http.server 8090 --bind 127.0.0.1 --directory "$dir/workspace" >/dev/null 2>&1 &
fi

python3 "$dir/fake_core.py" 8080 /etc/ocinye/instance-session.pub "$idle" &
for _ in 1 2 3 4 5 6 7 8 9 10; do [ -s /etc/ocinye/instance-session.pub ] && break; sleep 0.5; done
/opt/sx/ocinye-system-service serve &
OCINYE_CORE_API=http://127.0.0.1:8080/api/v1 /opt/sx/ocinye-session-service serve &
seatd -g video &
sleep 1
exec runuser -u ocinye-ui -- env ${SX_ENV:-} XDG_RUNTIME_DIR=/run/sx LIBSEAT_BACKEND=seatd HOME=/home/ocinye-ui \
    /opt/sx/ocinye-compositor -- /opt/sx/ocinye-system-ui
