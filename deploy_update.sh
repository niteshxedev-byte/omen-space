#!/usr/bin/env bash
set -e

if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root (use pkexec or sudo)" 
   exit 1
fi

echo "Stopping daemon..."
systemctl stop omen-space-daemon.service || true
killall -q omen-gui omen-tray omen-overlay omen-cli || true

echo "Installing updated binaries..."
install -m 755 /home/niteshxe.dev/omen-space/target/release/omen-space-daemon /usr/libexec/omen-space/omen-space-daemon
install -m 755 /home/niteshxe.dev/omen-space/target/release/omen-cli /usr/bin/omen-cli
install -m 755 /home/niteshxe.dev/omen-space/target/release/omen-gui /usr/bin/omen-gui
install -m 755 /home/niteshxe.dev/omen-space/target/release/omen-tray /usr/bin/omen-tray
install -m 755 /home/niteshxe.dev/omen-space/target/release/omen-overlay /usr/bin/omen-overlay

echo "Restarting daemon..."
systemctl daemon-reload
systemctl restart omen-space-daemon.service
echo "OMEN Space successfully deployed and restarted."
