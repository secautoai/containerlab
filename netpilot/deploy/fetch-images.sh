#!/bin/bash
# Download the network-OS images that are legitimately free and directly
# downloadable, and install them into NetPilot's image store so the matching
# device templates work out of the box — no manual firmware upload.
#
# Covered (free / public):
#   linux    Alpine Linux           (generic host/endpoint)
#   openwrt  OpenWrt x86-64         (router / firewall, LuCI)
#   chr      MikroTik RouterOS CHR  (free CHR; 1 Mbit/s unlicensed, fine for labs)
#   srlinux  Nokia SR Linux         (public container, pulled by NetPilot)
#
# NOT covered — proprietary NOS behind a vendor login / license, which cannot
# be fetched automatically: Cisco IOSv/IOS-XE/IOS-XR, Arista vEOS/cEOS,
# Juniper vSRX/vJunos/cRPD, Palo Alto PAN-OS, Fortinet FortiGate. Download
# those from the vendor and drop them in <data>/images/<template>/<version>/.
# VyOS ships only a rolling ISO for free (no qcow2) — install it by hand.
#
# Usage:  DATA=/var/lib/netpilot netpilot/deploy/fetch-images.sh
set -uo pipefail

DATA="${DATA:-/var/lib/netpilot}"
IMG="$DATA/images"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
SUDO="sudo"; [ "$(id -u)" = 0 ] && SUDO=""

OPENWRT_VER="${OPENWRT_VER:-24.10.7}"
CHR_VER="${CHR_VER:-7.23.1}"
ALPINE_REL="${ALPINE_REL:-3.22}"
ALPINE_IMG="${ALPINE_IMG:-nocloud_alpine-3.22.4-x86_64-bios-tiny-r0.qcow2}"
SRLINUX_REF="${SRLINUX_REF:-ghcr.io/nokia/srlinux:24.10.1}"

have() { command -v "$1" >/dev/null 2>&1; }
install_qcow() {  # <template> <version> <src-qcow2>
  local t="$1" v="$2" src="$3"
  $SUDO mkdir -p "$IMG/$t/$v"
  $SUDO cp "$src" "$IMG/$t/$v/$t-$v.qcow2"
  echo "  ✓ $t/$v installed ($(du -h "$IMG/$t/$v/$t-$v.qcow2" | cut -f1))"
}

echo "== Alpine Linux → linux/$ALPINE_REL =="
if curl -fsSL -m 600 -o "$TMP/alpine.qcow2" \
   "https://dl-cdn.alpinelinux.org/alpine/v$ALPINE_REL/releases/cloud/$ALPINE_IMG"; then
  install_qcow linux "$ALPINE_REL" "$TMP/alpine.qcow2"
else echo "  ✗ Alpine download failed"; fi

echo "== OpenWrt → openwrt/$OPENWRT_VER =="
if curl -fsSL -m 600 -o "$TMP/owrt.img.gz" \
   "https://downloads.openwrt.org/releases/$OPENWRT_VER/targets/x86/64/openwrt-$OPENWRT_VER-x86-64-generic-ext4-combined.img.gz"; then
  gunzip -f "$TMP/owrt.img.gz" || true
  if have qemu-img; then
    qemu-img convert -f raw -O qcow2 "$TMP/owrt.img" "$TMP/owrt.qcow2" && install_qcow openwrt "$OPENWRT_VER" "$TMP/owrt.qcow2"
  else echo "  ✗ qemu-img missing (install qemu-utils)"; fi
else echo "  ✗ OpenWrt download failed"; fi

echo "== MikroTik RouterOS CHR → chr/$CHR_VER =="
if curl -fsSL -m 600 -o "$TMP/chr.img.zip" \
   "https://download.mikrotik.com/routeros/$CHR_VER/chr-$CHR_VER.img.zip"; then
  ( cd "$TMP" && unzip -oq chr.img.zip )
  RAW="$(ls "$TMP"/chr-*.img 2>/dev/null | head -1)"
  if [ -n "$RAW" ] && have qemu-img; then
    qemu-img convert -f raw -O qcow2 "$RAW" "$TMP/chr.qcow2" && install_qcow chr "$CHR_VER" "$TMP/chr.qcow2"
  else echo "  ✗ CHR extract/convert failed"; fi
else echo "  ✗ CHR download failed"; fi

echo "== Nokia SR Linux (container) $SRLINUX_REF =="
if have docker; then
  if $SUDO docker pull "$SRLINUX_REF" >/dev/null 2>&1; then
    echo "  ✓ pulled $SRLINUX_REF ($($SUDO docker image inspect "$SRLINUX_REF" --format '{{.Architecture}}' 2>/dev/null))"
  else echo "  ✗ docker pull failed"; fi
else echo "  ✗ docker missing (container templates need docker.io)"; fi

echo
echo "Installed image store:"
$SUDO find "$IMG" -name '*.qcow2' -printf '  %p (%s bytes)\n' 2>/dev/null | sed "s#$IMG/##"
echo "Restart netpilot (or it rescans on next request) to pick these up."
