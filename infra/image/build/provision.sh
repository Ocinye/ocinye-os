#!/usr/bin/env bash
# Ocinye OS image build — what runs *inside* the build VM (D013 B05–B10).
#
# Driven by ocinye-image-builder over SSH, as root, one stage per boot:
#
#   provision.sh common   apt snapshot, upgrade, Docker (held, disabled), the
#                         preloaded images, the release payload, ocinye-firstboot,
#                         the configuration fragments, the accounts
#   provision.sh virt     kernel for VMs; cloud-init kept (allow-list)
#   provision.sh metal    generic kernel + firmware + UEFI boot packages; cloud-init off
#   provision.sh oie      the installation environment's live root (casper, curtin, ocinye-oie)
#   provision.sh finalize clean everything a clone must not share, then power off
#
# Inputs are files under /root/ocinye-build (uploaded by the builder):
#   build.env            ARCH, APT_SNAPSHOT, RELEASE_ID, DOCKER_KEY_FPR, BUILD_USER
#   release/             the D011 bundle without images/ (MANIFEST.json, install/, …)
#   images/*.tar         the release images (docker save)
#   third-party.txt      one `repo:tag@sha256:digest` per line
#   bin/                 ocinye-firstboot, ocinye-oie (static, target arch)
#   rootfs.tar           infra/image/rootfs
# Outputs under /root/ocinye-build/out (read back by the builder before finalize):
#   oci.json  dpkg-runtime.txt
#
# Nothing here reads a secret, and nothing it writes into the image is one.
set -euo pipefail
B=/root/ocinye-build
# shellcheck source=/dev/null
. "$B/build.env"
export DEBIAN_FRONTEND=noninteractive
APT=(apt-get -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold -y)
SNAPSHOT_URI="https://snapshot.ubuntu.com/ubuntu/$APT_SNAPSHOT"
SOURCES=/etc/apt/sources.list.d/ubuntu.sources
case "$ARCH" in arm64) SERIAL=ttyAMA0 ;; *) SERIAL=ttyS0 ;; esac
log() { printf '[provision %s] %s\n' "$STAGE" "$*"; }
STAGE="${1:?stage}"
mkdir -p "$B/out"

# No service may start inside the build (apt postinst scripts honour this).
policy_on() { printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d; chmod 755 /usr/sbin/policy-rc.d; }
policy_off() { rm -f /usr/sbin/policy-rc.d; }

apt_update() {
  # A fetch that fails is an error, never a warning over an empty index.
  local i
  for i in 1 2 3; do
    "${APT[@]}" update --error-on=any && return 0
    sleep 10
  done
  echo "PACKAGE_INSTALL_FAILED apt-update" >&2
  return 22
}

stage_common() {
  log "waiting for cloud-init and the network"
  cloud-init status --wait >/dev/null 2>&1 || true
  log "apt snapshot $APT_SNAPSHOT"
  # The build installs from the snapshot named in base.json (amd64 and arm64
  # both live there); the image keeps the archive sources it came with.
  [ -f "$B/ubuntu.sources.orig" ] || cp "$SOURCES" "$B/ubuntu.sources.orig"
  cat > "$SOURCES" <<SRC
Types: deb
URIs: $SNAPSHOT_URI
Suites: noble noble-updates noble-backports noble-security
Components: main universe restricted multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
SRC
  policy_on
  apt_update
  "${APT[@]}" dist-upgrade
  "${APT[@]}" install --no-install-recommends ufw sudo ca-certificates curl gnupg openssh-server cloud-init
  "${APT[@]}" purge snapd || true

  log "Docker repository (key fingerprint pinned by the release manifest)"
  install -d -m 0755 /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
  got="$(gpg --show-keys --with-colons /etc/apt/keyrings/docker.asc | awk -F: '/^fpr:/{print $10; exit}')"
  if [ "$got" != "$DOCKER_KEY_FPR" ]; then echo "RUNTIME_KEY_MISMATCH $got" >&2; exit 20; fi
  cat > /etc/apt/sources.list.d/docker.sources <<SRC
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: noble
Components: stable
Architectures: $ARCH
Signed-By: /etc/apt/keyrings/docker.asc
SRC
  apt-get update --error-on=any
  pins=()
  for p in docker-ce docker-ce-cli containerd.io docker-compose-plugin; do
    v="$(apt-cache policy "$p" | awk '/Candidate:/{print $2}')"
    [ -n "$v" ] && [ "$v" != "(none)" ] || { echo "PACKAGE_INSTALL_FAILED $p" >&2; exit 21; }
    pins+=("$p=$v"); echo "$p $v" >> "$B/out/dpkg-runtime.txt"
  done
  apt-get -y --no-install-recommends install "${pins[@]}"
  apt-mark hold docker-ce docker-ce-cli containerd.io docker-compose-plugin
  policy_off

  log "preloaded images"
  systemctl start containerd docker
  for t in "$B"/images/*.tar; do docker load -i "$t"; done
  while read -r ref; do
    [ -n "$ref" ] || continue
    repo_tag="${ref%@*}"; digest="${ref#*@}"
    last="${repo_tag##*/}"
    if [ "${last#*:}" != "$last" ]; then repo="${repo_tag%:*}"; else repo="$repo_tag"; fi
    docker pull "$repo@$digest"
    # The readable tag only when the reference has one: a digest-only
    # reference is pulled by digest and never gains a floating tag.
    if [ "$repo" != "$repo_tag" ]; then docker tag "$repo@$digest" "$repo_tag"; fi
  done < "$B/third-party.txt"
  docker image ls --digests --no-trunc --format '{{json .}}' > "$B/out/oci.json"
  systemctl stop docker.socket docker containerd
  systemctl disable docker.socket docker containerd
  rm -f /var/lib/docker/engine-id

  log "release payload and ocinye-firstboot"
  dest="/usr/lib/ocinye/release/$RELEASE_ID"
  rm -rf "$dest"; install -d -m 0755 "$dest"
  cp -a "$B/release/." "$dest/"
  chown -R root:root "$dest"; chmod -R go-w "$dest"
  install -m 0755 "$B/bin/ocinye-firstboot" /usr/lib/ocinye/ocinye-firstboot
  tar -C / --no-same-owner -xf "$B/rootfs.tar"
  chmod 0440 /etc/sudoers.d/60-ocinye-image
  visudo -c

  log "accounts"
  id ocinye >/dev/null 2>&1 || useradd -m -s /bin/bash -U ocinye
  passwd -l ocinye
  install -d -m 0700 -o ocinye -g ocinye /home/ocinye/.ssh
  : > /home/ocinye/.ssh/authorized_keys; chown ocinye:ocinye /home/ocinye/.ssh/authorized_keys; chmod 600 /home/ocinye/.ssh/authorized_keys
  id ocinye-claim >/dev/null 2>&1 || useradd -M -d /nonexistent -s /bin/sh ocinye-claim
  usermod -p '*' ocinye-claim
  id ocinye-claim-akc >/dev/null 2>&1 || useradd -r -M -d /nonexistent -s /usr/sbin/nologin ocinye-claim-akc

  # The build user keeps SSH access until finalize (AllowUsers accumulates).
  printf 'AllowUsers %s\n' "$BUILD_USER" > /etc/ssh/sshd_config.d/00-ocinye-build.conf

  log "units (first boot is enabled at finalize, never during the build)"
  systemctl disable unattended-upgrades.service motd-news.timer 2>/dev/null || true
  systemctl mask apt-news.service esm-cache.service 2>/dev/null || true
  install -d /etc/default/grub.d
  printf 'GRUB_CMDLINE_LINUX_DEFAULT="console=tty0 console=%s,115200n8"\nGRUB_TIMEOUT=3\nGRUB_TIMEOUT_STYLE=menu\n' "$SERIAL" > /etc/default/grub.d/90-ocinye.cfg
}

purge_kvm_kernel() {
  mapfile -t kvm < <(dpkg-query -W -f='${Package}\n' 'linux-*kvm*' 2>/dev/null || true)
  if [ "${#kvm[@]}" -gt 0 ]; then "${APT[@]}" purge "${kvm[@]}"; fi
}

stage_virt() {
  policy_on
  apt_update
  "${APT[@]}" install --no-install-recommends linux-virtual initramfs-tools
  purge_kvm_kernel
  policy_off
  update-grub
}

stage_metal() {
  policy_on
  apt_update
  # The complete in-target UEFI boot stack that curtin's install_missing_packages
  # requests for a debian UEFI install (curtin 24.0.0 curthooks.py): efibootmgr,
  # the real grub-efi-<arch> (unconditionally requested), its -bin, and the
  # -signed flavour, plus shim-signed. curtin only fetches what is NOT already
  # installed, so pre-installing the whole set here makes the offline ISO install
  # do zero apt/network work. grub-efi-<arch> was previously left out, and the
  # minimal base's grub-pc satisfied grub-efi-amd64-signed's
  # "grub-efi-amd64 | grub-pc" alternative in its place — so curtin later tried to
  # fetch grub-efi-amd64 with no NIC and the install failed.
  boot_pkgs=(efibootmgr)
  case "$ARCH" in
    amd64) boot_pkgs+=(grub-efi-amd64 grub-efi-amd64-bin grub-efi-amd64-signed shim-signed) ;;
    arm64) boot_pkgs+=(grub-efi-arm64 grub-efi-arm64-bin grub-efi-arm64-signed shim-signed) ;;
  esac
  # initramfs-tools explicitly: the minimal base boots without an initrd, and
  # real hardware needs one for its storage drivers.
  "${APT[@]}" install --no-install-recommends linux-generic linux-firmware initramfs-tools "${boot_pkgs[@]}"
  # D013 v1 is UEFI-only (BIOS legado NOT_SUPPORTED_V1): the BIOS GRUB flavour
  # must not linger in the metal rootfs. Remove it only now that grub-efi-<arch>
  # is installed, so the signed package's alternative dependency stays satisfied.
  if [ "$ARCH" = amd64 ]; then
    mapfile -t bios_grub < <(dpkg-query -W -f='${Package}\n' grub-pc grub-pc-bin 2>/dev/null || true)
    if [ "${#bios_grub[@]}" -gt 0 ]; then "${APT[@]}" purge "${bios_grub[@]}"; fi
  fi
  purge_kvm_kernel
  policy_off
  touch /etc/cloud/cloud-init.disabled
  update-grub
}

stage_oie() {
  policy_on
  apt_update
  "${APT[@]}" install --no-install-recommends linux-generic casper dosfstools e2fsprogs gdisk parted efibootmgr rsync eject \
    lvm2 mdadm dmsetup lshw \
    python3-yaml python3-pyudev python3-debian python3-oauthlib python3-jsonschema
  # curtin: the upstream tree at the commit pinned in base.json (noble has no
  # curtin package). Installation environment only, never in a target image.
  rm -rf /usr/lib/ocinye/curtin
  tar -C /usr/lib/ocinye --no-same-owner -xf "$B/curtin.tar"
  printf '#!/bin/sh\nexec /usr/lib/ocinye/curtin/bin/curtin "$@"\n' > /usr/local/bin/curtin
  chmod 755 /usr/local/bin/curtin
  curtin --help >/dev/null
  purge_kvm_kernel
  policy_off
  install -m 0755 "$B/bin/ocinye-oie" /usr/lib/ocinye/ocinye-oie
  # The OIE owns tty1 and ttyS0; no login prompt, no first boot, no SSH
  # (disabled at finalize, once the builder no longer needs it).
  cat > /etc/systemd/system/ocinye-oie@.service <<'UNIT'
[Unit]
Description=Ocinye Installation Environment on %I
After=systemd-user-sessions.service
Conflicts=getty@%i.service serial-getty@%i.service
Before=getty.target

[Service]
ExecStart=/bin/sh -c 'case %I in ttyS*|ttyAMA*) exec /usr/lib/ocinye/ocinye-oie run --serial ;; *) exec /usr/lib/ocinye/ocinye-oie run --vt ;; esac'
StandardInput=tty
StandardOutput=tty
TTYPath=/dev/%I
TTYReset=yes
Restart=always
RestartSec=2

[Install]
WantedBy=getty.target
UNIT
  # casper's initramfs, built from this root.
  update-initramfs -u -k all
}

stage_finalize() {
  local profile="${2:?profile}"
  log "finalize $profile: what a clone must not share"
  # The binaries and fragments of this run (a reused common layer may carry
  # older ones).
  install -m 0755 "$B/bin/ocinye-firstboot" /usr/lib/ocinye/ocinye-firstboot
  tar -C / --no-same-owner -xf "$B/rootfs.tar"
  chmod 0440 /etc/sudoers.d/60-ocinye-image
  visudo -c >/dev/null
  case "$profile" in
    virt|metal)
      systemctl enable ocinye-firstboot.service ocinye-console@tty1.service "ocinye-console@$SERIAL.service" ;;
    oie)
      systemctl disable ssh.service ssh.socket cloud-init.service cloud-init-local.service cloud-config.service cloud-final.service 2>/dev/null || true
      systemctl enable ocinye-oie@tty1.service "ocinye-oie@$SERIAL.service"
      touch /etc/cloud/cloud-init.disabled ;;
  esac
  rm -f /etc/ssh/sshd_config.d/00-ocinye-build.conf
  rm -rf /var/lib/ocinye-firstboot
  "${APT[@]}" purge apport popularity-contest 2>/dev/null || true
  "${APT[@]}" autoremove --purge || true
  # The package inventory, while the indexes that give each package its
  # archive digest and origin are still here.
  : > "$B/out/packages.tsv"
  dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\t${Status}\n' | while IFS=$'\t' read -r n v a st; do
    case "$st" in *" installed") ;; *) continue ;; esac
    spec="$n=$v"; [ "$a" = all ] || spec="$n:$a=$v"
    h="$(apt-cache show --no-all-versions "$spec" 2>/dev/null | awk '/^SHA256:/{print $2; exit}')"
    src="$(apt-cache policy "$n:$a" 2>/dev/null | awk '/\*\*\*/{f=1; next} f && /http/{print $2 " " $3; exit}')"
    held=0; case "$st" in hold*) held=1 ;; esac
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$n" "$v" "$a" "${h:--}" "${src:--}" "$held" >> "$B/out/packages.tsv"
  done
  apt-get clean
  rm -rf /var/lib/apt/lists/*
  # apt sources back to the archive the base image had.
  if [ -f "$B/ubuntu.sources.orig" ]; then cp "$B/ubuntu.sources.orig" "$SOURCES"; fi
  cloud-init clean --logs --seed || true
  rm -rf /var/lib/cloud/* /var/log/cloud-init*
  rm -f /etc/ssh/ssh_host_*
  rm -f /etc/netplan/50-cloud-init.yaml /etc/cloud/cloud.cfg.d/90-installer-network.cfg
  rm -rf /var/lib/systemd/network/* /var/lib/dhcp/* /run/systemd/netif/leases/* 2>/dev/null || true
  rm -f /var/lib/systemd/random-seed /var/lib/systemd/credential.secret
  rm -rf /var/lib/docker/network/files/* /var/lib/docker/containers/* /var/lib/docker/volumes/* /var/lib/docker/buildkit 2>/dev/null || true
  rm -f /var/lib/docker/engine-id
  rm -rf /tmp/* /var/tmp/* /root/.bash_history /root/.cache /root/.ssh
  # The build user (whose session this is) is removed offline, at B11.
  journalctl --rotate >/dev/null 2>&1 || true
  journalctl --vacuum-time=1s >/dev/null 2>&1 || true
  find /var/log -type f -exec truncate -s 0 {} + 2>/dev/null || true
  : > /etc/machine-id
  rm -f /var/lib/dbus/machine-id
  printf 'localhost\n' > /etc/hostname
  sync
}

case "$STAGE" in
  common) stage_common ;;
  virt) stage_virt ;;
  metal) stage_metal ;;
  oie) stage_oie ;;
  finalize) stage_finalize "$@" ;;
  *) echo "unknown stage $STAGE" >&2; exit 2 ;;
esac
log done
