#!/bin/sh
# Removing WhiteRose (purge) gives the network back to how it was.
set -e
if [ "$1" = "purge" ]; then
    rm -f /etc/netplan/90-whiterose-network-manager.yaml
fi
exit 0
