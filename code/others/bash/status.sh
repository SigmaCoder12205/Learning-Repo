#!/usr/bin/env bash
# vpn-tailscale-status.sh
# Quick snapshot of Tailscale + Proton VPN state on Omarchy (Arch/Hyprland).
# Run with: bash vpn-tailscale-status.sh

set -uo pipefail

hr() { printf '\n\033[1;34m== %s ==\033[0m\n' "$1"; }

hr "Tailscale status"
if command -v tailscale >/dev/null 2>&1; then
    tailscale status
    echo
    echo "Tailscale IP(s):"
    tailscale ip -4 2>/dev/null
    tailscale ip -6 2>/dev/null
else
    echo "tailscale binary not found on PATH"
fi

hr "Proton VPN status"
if command -v protonvpn-cli >/dev/null 2>&1; then
    protonvpn-cli status
elif systemctl is-active --quiet protonvpn 2>/dev/null; then
    echo "protonvpn systemd service is active"
else
    echo "No protonvpn-cli found and no protonvpn service detected."
    echo "If you're using the Proton VPN GUI app, check its tray icon status manually."
fi

hr "Network interfaces"
ip -brief address show

hr "Routing table"
ip route show

hr "Which interface is the default route actually using?"
ip route get 1.1.1.1

hr "DNS resolver in use"
if command -v resolvectl >/dev/null 2>&1; then
    resolvectl status | grep -A3 "Current DNS Server\|DNS Servers" 
else
    cat /etc/resolv.conf
fi

hr "Public-facing IP (what the internet sees)"
curl -s --max-time 5 https://am.i.mullvad.net/ip || curl -s --max-time 5 https://ifconfig.me
echo

hr "Tailscale's view of exit node / relay usage"
tailscale status --json 2>/dev/null | grep -E '"ExitNodeID"|"Relay"' || echo "tailscale JSON status unavailable"