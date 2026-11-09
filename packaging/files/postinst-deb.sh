#!/bin/sh
# After installing on Ubuntu: let NetworkManager run the network, as Ubuntu
# Desktop does, so Wi-Fi and the Network page work (Ubuntu Server hands it to
# systemd-networkd). Takes effect at the next boot; nothing is restarted now,
# so a connection in use isn't dropped.
set -e
if [ -d /etc/netplan ] && ! grep -rqs "renderer: *NetworkManager" /etc/netplan; then
    cat > /etc/netplan/90-whiterose-network-manager.yaml <<'YAML'
# Added by WhiteRose: NetworkManager runs the network (Wi-Fi, the Network page).
network:
  version: 2
  renderer: NetworkManager
YAML
    chmod 600 /etc/netplan/90-whiterose-network-manager.yaml
fi
exit 0
