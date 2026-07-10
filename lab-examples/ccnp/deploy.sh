#!/usr/bin/env bash
# deploy.sh - one entry point for the CCNP containerlab study labs.
#
#   ./deploy.sh check                 verify prerequisites and images
#   ./deploy.sh list                  list labs and their deployment state
#   ./deploy.sh deploy <lab> [opts]   deploy a lab (opts: --solved --reconfigure)
#   ./deploy.sh destroy <lab>|all     destroy a lab, keep saved configs (NVRAM)
#   ./deploy.sh reset <lab>           destroy AND wipe saved state -> back to baseline
#   ./deploy.sh redeploy <lab>        destroy (keep state) + deploy
#   ./deploy.sh status [lab]          containerlab inspect for one lab / all labs
#   ./deploy.sh save <lab>            save running-config -> startup on every node
#   ./deploy.sh ssh <lab> <node>      ssh into a node (admin/admin for IOL)
#   ./deploy.sh graph <lab>           serve the topology graph on :50080
#
# <lab> may be given as: lab02 | 02 | 2 | lab02-ospf
#
# Environment overrides (also see README.md):
#   CCNP_IOL_IMAGE      L3 router image  (default vrnetlab/cisco_iol:17.12.01)
#   CCNP_IOL_L2_IMAGE   L2 switch image  (default vrnetlab/cisco_iol:L2-17.12.01)
#   CCNP_CFG            configs | solutions (set automatically by --solved)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# defaults must match the ${CCNP_...:=...} defaults inside the .clab.yml files
CCNP_IOL_IMAGE="${CCNP_IOL_IMAGE:-vrnetlab/cisco_iol:17.12.01}"
CCNP_IOL_L2_IMAGE="${CCNP_IOL_L2_IMAGE:-vrnetlab/cisco_iol:L2-17.12.01}"
FREE_IMAGES=("quay.io/frrouting/frr:10.5.1" "wbitt/network-multitool:3.22.2")

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
info()  { echo "${BLUE}[info]${NC} $*"; }
ok()    { echo "${GREEN}[ ok ]${NC} $*"; }
warn()  { echo "${YELLOW}[warn]${NC} $*"; }
fail()  { echo "${RED}[fail]${NC} $*" >&2; }
die()   { fail "$*"; exit 1; }

# run containerlab (root required) preserving the CCNP_* environment
CLAB_BIN="${CLAB_BIN:-containerlab}"
clab() {
    local sudo_cmd=()
    if [ "$(id -u)" -ne 0 ]; then sudo_cmd=(sudo -E); fi
    "${sudo_cmd[@]}" env \
        "CCNP_IOL_IMAGE=$CCNP_IOL_IMAGE" \
        "CCNP_IOL_L2_IMAGE=$CCNP_IOL_L2_IMAGE" \
        "CCNP_CFG=${CCNP_CFG:-configs}" \
        "$CLAB_BIN" "$@"
}

# ---------------------------------------------------------------- lab lookup
lab_dirs() { find . -maxdepth 1 -type d -name 'lab[0-9][0-9]-*' | sort | sed 's|^\./||'; }

resolve_lab() {  # accepts lab02 | 02 | 2 | lab02-ospf -> echoes directory name
    local arg="${1:-}" num
    [ -n "$arg" ] || die "missing <lab> argument (try: $0 list)"
    [ -d "$arg" ] && { echo "${arg%/}"; return; }
    num="${arg#lab}"
    [[ "$num" =~ ^[0-9]+$ ]] || die "unknown lab '$arg' (try: $0 list)"
    num="$(printf '%02d' "$((10#$num))")"
    local match
    match="$(lab_dirs | grep -E "^lab${num}-" || true)"
    [ -n "$match" ] || die "no lab directory matches 'lab${num}-*'"
    echo "$match"
}

topo_file() {  # lab dir -> topology file path
    local f
    f="$(find "$1" -maxdepth 1 -name '*.clab.yml' | head -n1)"
    [ -n "$f" ] || die "no .clab.yml found in $1"
    echo "$f"
}

lab_name() { sed -n 's/^name:[[:space:]]*//p' "$(topo_file "$1")" | head -n1; }

deployed() {  # true if the lab has running containers
    docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^clab-$(lab_name "$1")-" || return 1
}

uses_image() { grep -q "$1" "$(topo_file "$2")"; }

# ---------------------------------------------------------------- commands
cmd_check() {
    local rc=0
    if command -v "$CLAB_BIN" >/dev/null 2>&1; then
        ok "containerlab: $($CLAB_BIN version 2>/dev/null | sed -n 's/.*version:[[:space:]]*//Ip' | head -n1 || echo installed)"
    else
        fail "containerlab not found - see TUTORIAL.md section 2"; rc=1
    fi
    if docker info >/dev/null 2>&1; then
        ok "docker daemon reachable"
    else
        fail "docker daemon not reachable (is docker installed / are you in the docker group?)"; rc=1
    fi
    [ "$(uname -m)" = "x86_64" ] && ok "architecture: x86_64" || { fail "architecture $(uname -m): Cisco IOL requires x86_64"; rc=1; }

    local img
    for img in "${FREE_IMAGES[@]}"; do
        if docker image inspect "$img" >/dev/null 2>&1; then
            ok "image present: $img"
        else
            warn "image not local (pulled automatically on deploy): $img"
        fi
    done
    for img in "$CCNP_IOL_IMAGE" "$CCNP_IOL_L2_IMAGE"; do
        if docker image inspect "$img" >/dev/null 2>&1; then
            ok "image present: $img"
        else
            warn "image missing: $img  -> labs using it will not deploy (TUTORIAL.md section 3)"
        fi
    done
    return $rc
}

cmd_list() {
    printf '%-24s %-10s %s\n' "LAB" "STATE" "TITLE"
    local d state title
    for d in $(lab_dirs); do
        state="-"
        deployed "$d" && state="${GREEN}running${NC}" || state="stopped"
        title="$(sed -n 's/^# //p' "$d/README.md" 2>/dev/null | head -n1)"
        printf '%-24s %-10b %s\n' "$d" "$state" "$title"
    done
    echo
    echo "hint: '$0 deploy <lab>' then open <lab>/README.md and follow the tutorial"
}

cmd_deploy() {
    local dir; dir="$(resolve_lab "$1")"; shift
    local extra=()
    export CCNP_CFG="${CCNP_CFG:-configs}"
    while [ $# -gt 0 ]; do
        case "$1" in
            --solved)      export CCNP_CFG=solutions ;;
            --reconfigure) extra+=(--reconfigure) ;;
            *) die "unknown option '$1'" ;;
        esac
        shift
    done
    # refuse to deploy an IOL lab when the image is missing - clearer than clab's error
    if uses_image '\${CCNP_IOL_IMAGE' "$dir" && ! docker image inspect "$CCNP_IOL_IMAGE" >/dev/null 2>&1; then
        die "$dir needs $CCNP_IOL_IMAGE - build it first (TUTORIAL.md section 3)"
    fi
    if uses_image '\${CCNP_IOL_L2_IMAGE' "$dir" && ! docker image inspect "$CCNP_IOL_L2_IMAGE" >/dev/null 2>&1; then
        die "$dir needs $CCNP_IOL_L2_IMAGE - build it first (TUTORIAL.md section 3)"
    fi
    [ "$CCNP_CFG" = "solutions" ] && warn "deploying with SOLUTION configs (--solved)"
    info "deploying $dir ..."
    clab deploy -t "$(topo_file "$dir")" "${extra[@]}"
    echo
    ok "deployed. Next: open $dir/README.md and start the tasks."
    echo "     ssh: $0 ssh ${dir%%-*} <node>   (IOL credentials: admin/admin)"
}

cmd_destroy() {
    if [ "${1:-}" = "all" ]; then
        local d
        for d in $(lab_dirs); do
            deployed "$d" && { info "destroying $d"; clab destroy -t "$(topo_file "$d")"; }
        done
        ok "all labs destroyed (saved configs kept)"
        return
    fi
    local dir; dir="$(resolve_lab "${1:-}")"
    clab destroy -t "$(topo_file "$dir")"
    ok "$dir destroyed - saved device configs kept; 'deploy' resumes where you left off"
}

cmd_reset() {
    local dir; dir="$(resolve_lab "${1:-}")"
    clab destroy -t "$(topo_file "$dir")" --cleanup
    ok "$dir wiped - next deploy boots the baseline configs from $dir/configs/"
}

cmd_redeploy() {
    local dir; dir="$(resolve_lab "${1:-}")"
    deployed "$dir" && clab destroy -t "$(topo_file "$dir")"
    cmd_deploy "$dir"
}

cmd_status() {
    if [ -n "${1:-}" ]; then
        clab inspect -t "$(topo_file "$(resolve_lab "$1")")"
    else
        clab inspect --all 2>/dev/null || info "no labs running"
    fi
}

cmd_save() {
    local dir; dir="$(resolve_lab "${1:-}")"
    deployed "$dir" || die "$dir is not running"
    clab save -t "$(topo_file "$dir")"
    ok "running-config saved to startup on all $dir nodes"
}

cmd_ssh() {
    local dir node; dir="$(resolve_lab "${1:-}")"
    node="${2:-}"; [ -n "$node" ] || die "usage: $0 ssh <lab> <node>   e.g. $0 ssh 2 r1"
    exec ssh "admin@clab-$(lab_name "$dir")-$node"
}

cmd_graph() {
    local dir; dir="$(resolve_lab "${1:-}")"
    info "topology graph on http://localhost:50080 - Ctrl-C to stop"
    clab graph -t "$(topo_file "$dir")"
}

usage() { sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; }

case "${1:-}" in
    check)    shift; cmd_check "$@" ;;
    list)     shift; cmd_list "$@" ;;
    deploy)   shift; cmd_deploy "$@" ;;
    destroy)  shift; cmd_destroy "$@" ;;
    reset)    shift; cmd_reset "$@" ;;
    redeploy) shift; cmd_redeploy "$@" ;;
    status)   shift; cmd_status "${1:-}" ;;
    save)     shift; cmd_save "$@" ;;
    ssh)      shift; cmd_ssh "$@" ;;
    graph)    shift; cmd_graph "$@" ;;
    -h|--help|help|"") usage ;;
    *) usage; die "unknown command '${1}'" ;;
esac
