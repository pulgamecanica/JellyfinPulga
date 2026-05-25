#!/bin/bash
set -e

PLUGIN_DIR="/var/lib/jellyfin/plugins/JellyfinPulga_0.1.0.0"

echo "Installing JellyfinPulga plugin..."
sudo mkdir -p "$PLUGIN_DIR"
sudo cp ~/JellyfinPulga.dll ~/meta.json "$PLUGIN_DIR/"
sudo chown -R jellyfin:jellyfin "$PLUGIN_DIR"
echo "Plugin files installed."

echo "Restarting Jellyfin..."
sudo systemctl restart jellyfin
echo "Done! Check the Jellyfin dashboard for the JellyfinPulga plugin."
