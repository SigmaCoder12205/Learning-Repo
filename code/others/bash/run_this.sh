#!/usr/bin/env bash
# vpn-tailscale-leak-test.sh
# Checks for the two most common Proton VPN + Tailscale conflicts:
#   1. DNS leaking outside the VPN tunnel
#   2. Tailscale peer traffic breaking (or being routed) unexpectedly when Proton is up
# Run with: bash vpn-tailscale-leak-test.sh

set -uo pipefail

hr() { printf '\n\033[1;34m== %s ==\033[0m\n' "$1"; }
pass() { printf '\033[1;32mOK:\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33mCHECK:\033[0m %s\n' "$1"; }

hr "1. DNS leak test"
echo "Comparing your public IP to the resolver your DNS queries actually go through."
PUB_IP=$(curl -s --max-time 5 https://ifconfig.me)
echo "Public IP (via HTTP):        $PUB_IP"

DNS_LEAK_IP=$(curl -s --max-time 5 "https://dnsleaktest.com/" >/dev/null 2>&1; \
  dig +short whoami.akamai.net @ns1-1.akamaitech.net 2>/dev/null)
if [ -n "$DNS_LEAK_IP" ]; then
    echo "DNS resolver's apparent IP:  $DNS_LEAK_IP"
    if [ "$PUB_IP" = "$DNS_LEAK_IP" ]; then
        pass "DNS and HTTP exit through the same IP (no obvious leak)."
    else
        warn "DNS resolver IP differs from your public IP -- possible DNS leak outside the VPN."
    fi
else
    warn "Could not determine DNS exit IP automatically (dig/akamai lookup failed). Use https://dnsleaktest.com manually in a browser to be sure."
fi

hr "2. Is systemd-resolved pointing at Proton's DNS or your ISP's?"
if command -v resolvectl >/dev/null 2>&1; then
    resolvectl dns
else
    echo "resolvectl not available, showing /etc/resolv.conf:"
    cat /etc/resolv.conf
fi

hr "3. Tailscale peer reachability while Proton VPN is active"
if command -v tailscale >/dev/null 2>&1; then
    PEERS=$(tailscale status --json 2>/dev/null | grep -oP '"TailscaleIPs":\["\K[0-9.]+' | head -n 5)
    if [ -z "$PEERS" ]; then
        warn "No peers found in tailscale status. Is tailscale up and connected to other devices?"
    else
        for ip in $PEERS; do
            if ping -c 1 -W 2 "$ip" >/dev/null 2>&1; then
                pass "Reachable: $ip"
            else
                warn "NOT reachable: $ip -- Proton's routes may be shadowing Tailscale's subnet routes."
            fi
        done
    fi
else
    echo "tailscale binary not found."
fi

hr "4. Route priority check (does Proton override Tailscale's 100.64.0.0/10 range?)"
echo "Tailscale uses 100.64.0.0/10 by default. This should NOT be routed through the Proton tunnel."
ip route show | grep -E '100\.64\.0\.0/10|tailscale' || warn "No explicit Tailscale CGNAT route found -- check 'ip route show table all'"
echo
echo "Full route table for cross-reference:"
ip route show table all 2>/dev/null | grep -E 'tun|proton|tailscale|100\.64'

hr "5. Kill switch interference (if Proton's kill switch is on, it may block Tailscale's UDP)"
if command -v nft >/dev/null 2>&1; then
    echo "Active nftables rulesets mentioning tailscale or proton:"
    sudo nft list ruleset 2>/dev/null | grep -iE 'proton|tailscale|41641' || echo "Nothing matched -- can't confirm from nftables alone."
elif command -v iptables >/dev/null 2>&1; then
    sudo iptables -L -n | grep -iE 'proton|tailscale|41641' || echo "Nothing matched -- can't confirm from iptables alone."
fi

hr "Summary"
echo "If you saw any CHECK lines above, that's where Proton and Tailscale are likely stepping on each other."
echo "Most common real-world fix: exclude Tailscale's UDP port (41641) and the 100.64.0.0/10 range from Proton's tunnel/kill switch, or set Tailscale's routes with higher priority (lower metric) than the Proton tun interface."