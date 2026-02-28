# =============================================================================
# ZeroClaw Firejail Profile
# =============================================================================
# OS-level sandboxing using Firejail
#
# Installation:
#   sudo apt install firejail
#   sudo cp firejail_profile.profile /etc/firejail/zeroclaw.profile
#
# Usage:
#   firejail --profile=/etc/firejail/zeroclaw.profile /opt/zeroclaw/zeroclaw
#
# Or with the wrapper:
#   firejail --profile=/etc/firejail/zeroclaw.profile \
#     --env=ZEROCLAW_CONFIG=/etc/zeroclaw/config.toml \
#     /opt/zeroclaw/zeroclaw
#
# =============================================================================

# Profile metadata
quiet
name=zeroclaw

# -----------------------------------------------------------------------------
# Filesystem - Whitelist Approach (most restrictive)
# -----------------------------------------------------------------------------
# Start with nothing and add only what's needed
blacklist /

# Whitelist specific directories
whitelist /opt/zeroclaw
whitelist /etc/zeroclaw
whitelist /var/lib/zeroclaw
whitelist /var/log/zeroclaw
whitelist /usr/lib
whitelist /usr/lib64
whitelist /lib
whitelist /lib64
whitelist /usr/share/locale
whitelist /usr/share/zoneinfo

# Read-only directories
read-only /opt/zeroclaw
read-only /etc/zeroclaw
read-only /usr
read-only /lib
read-only /lib64

# Writable directories (workspace only)
# Note: /var/lib/zeroclaw is writable by default since it's whitelisted

# -----------------------------------------------------------------------------
# Blacklist Sensitive Locations (defense in depth)
# -----------------------------------------------------------------------------
blacklist /root
blacklist /home
blacklist /etc/passwd
blacklist /etc/shadow
blacklist /etc/sudoers
blacklist /etc/ssh
blacklist /etc/ssl/private
blacklist ~/.ssh
blacklist ~/.gnupg
blacklist ~/.aws
blacklist ~/.config
blacklist ~/.local
blacklist /boot
blacklist /mnt
blacklist /media
blacklist /srv
blacklist /run/user

# Block access to dangerous files
blacklist /proc/kcore
blacklist /proc/kmem
blacklist /proc/mem
blacklist /sys/firmware

# -----------------------------------------------------------------------------
# Capability Dropping
# -----------------------------------------------------------------------------
# Drop ALL capabilities (ZeroClaw needs none)
caps.drop all

# -----------------------------------------------------------------------------
# Seccomp Filtering
# -----------------------------------------------------------------------------
# Enable seccomp with default blacklist
seccomp

# Block dangerous syscalls explicitly
seccomp.drop mount,umount2,pivot_root,chroot
seccomp.drop ptrace,process_vm_readv,process_vm_writev
seccomp.drop kexec_load,kexec_file_load
seccomp.drop init_module,finit_module,delete_module
seccomp.drop reboot,sethostname,setdomainname
seccomp.drop swapon,swapoff
seccomp.drop acct,settimeofday,clock_settime,clock_adjtime
seccomp.drop create_module,query_module,get_kernel_syms

# -----------------------------------------------------------------------------
# Network Restrictions
# -----------------------------------------------------------------------------
# Allow only localhost networking
netfilter
net none

# If network access is needed, use:
# protocol unix,inet,inet6
# And add firewall rules for localhost only

# -----------------------------------------------------------------------------
# Namespace Isolation
# -----------------------------------------------------------------------------
# Use separate namespaces for additional isolation

# New IPC namespace
ipc-namespace

# New network namespace (with no network = localhost only)
netns

# New PID namespace
# Note: This may cause issues with child processes - test before enabling
# pid-namespace

# New mount namespace (implicit with private mounts)
private-tmp

# New user namespace (may require configuration)
# noroot

# -----------------------------------------------------------------------------
# Process Restrictions
# -----------------------------------------------------------------------------
# Disable no-new-privileges
nonewprivs

# No SUID/SGID binaries
nosound

# No access to /dev (except essential devices)
private-dev

# No dbus access
nodbus

# No video devices
no3d

# Disable X11 access
x11 none

# -----------------------------------------------------------------------------
# Memory and Resource Limits
# -----------------------------------------------------------------------------
# Memory limit (512MB)
rlimit-as 536870912

# Max file size (10MB)
rlimit-fsize 10485760

# Max open files
rlimit-nofile 1024

# Max processes
rlimit-nproc 50

# CPU limit (in seconds per day) - commented out, use cgroups instead
# rlimit-cpu 3600

# -----------------------------------------------------------------------------
# Filesystem Options
# -----------------------------------------------------------------------------
# Private /etc (copy from host, isolated)
# private-etc zeroclaw,localtime,timezone,hosts,resolv.conf

# Private /bin (minimal binaries)
# private-bin zeroclaw,sh,ls,cat,head,tail,grep

# Private /lib (only required libraries)
# Note: Auto-detected by Firejail

# -----------------------------------------------------------------------------
# DNS
# -----------------------------------------------------------------------------
# Use localhost DNS only (or disable if no network needed)
# dns 127.0.0.1

# -----------------------------------------------------------------------------
# Environment
# -----------------------------------------------------------------------------
# Clean environment
env ZEROCLAW_CONFIG=/etc/zeroclaw/config.toml
env RUST_BACKTRACE=0
env HOME=/var/lib/zeroclaw
env LANG=C.UTF-8
env LC_ALL=C.UTF-8
env PATH=/usr/local/bin:/usr/bin:/bin

# Remove sensitive environment variables
rmenv LD_PRELOAD
rmenv LD_LIBRARY_PATH
rmenv LD_AUDIT
rmenv HISTFILE
rmenv AWS_ACCESS_KEY_ID
rmenv AWS_SECRET_ACCESS_KEY
rmenv API_KEY
rmenv TOKEN

# -----------------------------------------------------------------------------
# Miscellaneous Security
# -----------------------------------------------------------------------------
# Shell: restricted to /bin/sh
shell none

# Disable debug
notv

# Machine ID: Randomize
machine-id

# Hostname: Use sandbox name
# hostname zeroclaw-sandbox
