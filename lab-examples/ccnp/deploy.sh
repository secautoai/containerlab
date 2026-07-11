#!/usr/bin/env bash
# deploy.sh - one entry point for the CCNP containerlab study labs.
#
#   ./deploy.sh check                 verify prerequisites and images
#   ./deploy.sh list                  list labs and their deployment state
#   ./deploy.sh deploy <lab> [opts]   deploy a lab (opts: --solved --reconfigure)
#   ./deploy.sh destroy <lab>|all     destroy a lab, keep saved configs (NVRAM)
#   ./deploy.sh reset <lab>           destroy AND wipe saved state -> back to baseline
#   ./deploy.sh redeploy <lab> [opts] destroy (keep state) + deploy
#   ./deploy.sh status [lab]          containerlab inspect for one lab / all labs
#   ./deploy.sh save <lab>            save running-config -> startup on every IOS node
#   ./deploy.sh ssh <lab> <node>      ssh into an IOS node (admin/admin)
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

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
info()  { echo "${BLUE}[info]${NC} $*"; }
ok()    { echo "${GREEN}[ ok ]${NC} $*"; }
warn()  { echo "${YELLOW}[warn]${NC} $*"; }
fail()  { echo "${RED}[fail]${NC} $*" >&2; }
die()   { fail "$*"; exit 1; }

# run containerlab (root required) preserving the CCNP_* environment.
# ${arr[@]+...} guards empty-array expansion for bash < 4.4 under set -u.
CLAB_BIN="${CLAB_BIN:-containerlab}"
clab() {
    local sudo_cmd=()
    if [ "$(id -u)" -ne 0 ]; then sudo_cmd=(sudo -E); fi
    ${sudo_cmd[@]+"${sudo_cmd[@]}"} env \
        "CCNP_IOL_IMAGE=$CCNP_IOL_IMAGE" \
        "CCNP_IOL_L2_IMAGE=$CCNP_IOL_L2_IMAGE" \
        "CCNP_CFG=${CCNP_CFG:-configs}" \
        "$CLAB_BIN" "$@"
}

# ---------------------------------------------------------------- lab lookup
lab_dirs() { find . -maxdepth 1 -type d -name 'lab[0-9][0-9]-*' | sort | sed 's|^\./||'; }

resolve_lab() {  # accepts lab02 | 02 | 2 | lab02-ospf -> echoes directory name
    local arg="${1:-}" num matches
    [ -n "$arg" ] || die "missing <lab> argument (try: $0 list)"
    [ -d "$arg" ] && { echo "${arg%/}"; return; }
    num="${arg#lab}"
    [[ "$num" =~ ^[0-9]+$ ]] || die "unknown lab '$arg' (try: $0 list)"
    num="$(printf '%02d' "$((10#$num))")"
    matches="$(lab_dirs | grep -E "^lab${num}-" || true)"
    [ -n "$matches" ] || die "no lab directory matches 'lab${num}-*'"
    if [ "$(wc -l <<<"$matches")" -gt 1 ]; then
        die "lab number '$arg' is ambiguous: $(tr '\n' ' ' <<<"$matches")- use the full directory name"
    fi
    echo "$matches"
}

topo_file() {  # lab dir -> topology file path
    local f
    f="$(find "$1" -maxdepth 1 -name '*.clab.yml' | head -n1)"
    [ -n "$f" ] || die "no .clab.yml found in $1"
    echo "$f"
}

lab_name() { sed -n 's/^name:[[:space:]]*//p' "$(topo_file "$1")" | head -n1; }

running_containers() {  # buffered docker ps snapshot (avoids grep -q SIGPIPE under pipefail)
    docker ps --format '{{.Names}}' 2>/dev/null || true
}

deployed() {  # deployed <labdir> [snapshot] -> true if the lab has running containers
    local snapshot="${2:-$(running_containers)}"
    grep -q "^clab-$(lab_name "$1")-" <<<"$snapshot"
}

is_ios_lab() { grep -q 'kind: cisco_iol' "$(topo_file "$1")"; }

topo_images() {  # every image a lab's topology can resolve to (env defaults expanded)
    sed -n 's/.*image:[[:space:]]*//p' "$(topo_file "$1")" \
        | sed -e "s|\${CCNP_IOL_IMAGE:=[^}]*}|$CCNP_IOL_IMAGE|" \
              -e "s|\${CCNP_IOL_L2_IMAGE:=[^}]*}|$CCNP_IOL_L2_IMAGE|" \
        | sort -u
}

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

    # check every image any lab topology resolves to (derived, not hardcoded)
    local d img
    for img in $(for d in $(lab_dirs); do topo_images "$d"; done | sort -u); do
        if docker image inspect "$img" >/dev/null 2>&1; then
            ok "image present: $img"
        elif [[ "$img" == vrnetlab/* ]]; then
            warn "image missing: $img  -> labs using it will not deploy (TUTORIAL.md section 3)"
        else
            warn "image not local (pulled automatically on deploy): $img"
        fi
    done
    return $rc
}

cmd_list() {
    printf '%-24s %-10s %s\n' "LAB" "STATE" "TITLE"
    local d state title snapshot
    snapshot="$(running_containers)"
    for d in $(lab_dirs); do
        if deployed "$d" "$snapshot"; then
            state="${GREEN}$(printf '%-10s' running)${NC}"
        else
            state="$(printf '%-10s' stopped)"
        fi
        title="$(sed -n 's/^# //p' "$d/README.md" 2>/dev/null | head -n1 || true)"
        printf '%-24s %b %s\n' "$d" "$state" "$title"
    done
    echo
    echo "hint: '$0 deploy <lab>' then open <lab>/README.md and follow the tutorial"
}

cmd_deploy() {
    local dir; dir="$(resolve_lab "${1:-}")"; shift || true
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
    if [ "$CCNP_CFG" = "solutions" ] && [ ! -d "$dir/solutions" ]; then
        die "$dir has no solutions/ directory (lab00 is baseline-only) - deploy without --solved"
    fi
    # refuse to deploy when a required image is missing - clearer than clab's error
    local img
    for img in $(topo_images "$dir"); do
        if [[ "$img" == vrnetlab/* ]] && ! docker image inspect "$img" >/dev/null 2>&1; then
            die "$dir needs $img - build it first (TUTORIAL.md section 3)"
        fi
    done
    [ "$CCNP_CFG" = "solutions" ] && warn "deploying with SOLUTION configs (--solved)"
    info "deploying $dir ..."
    clab deploy -t "$(topo_file "$dir")" ${extra[@]+"${extra[@]}"}
    echo
    ok "deployed. Next: open $dir/README.md and start the tasks."
    if is_ios_lab "$dir"; then
        echo "     ssh: $0 ssh ${dir%%-*} <node>   (IOS credentials: admin/admin)"
    else
        echo "     shell: docker exec -it clab-$(lab_name "$dir")-<node> bash   (vtysh on the routers)"
    fi
}

cmd_destroy() {
    if [ "${1:-}" = "all" ]; then
        local d snapshot
        snapshot="$(running_containers)"
        for d in $(lab_dirs); do
            deployed "$d" "$snapshot" && { info "destroying $d"; clab destroy -t "$(topo_file "$d")"; }
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
    local dir; dir="$(resolve_lab "${1:-}")"; shift || true
    deployed "$dir" && clab destroy -t "$(topo_file "$dir")"
    cmd_deploy "$dir" "$@"
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
    if ! is_ios_lab "$dir"; then
        die "$dir has no IOS nodes - 'clab save' cannot save FRR/linux nodes; use 'write' inside vtysh (writes the bind-mounted frr.conf)"
    fi
    clab save -t "$(topo_file "$dir")"
    ok "running-config saved to startup on all $dir IOS nodes"
}

cmd_ssh() {
    local dir node; dir="$(resolve_lab "${1:-}")"
    node="${2:-}"; [ -n "$node" ] || die "usage: $0 ssh <lab> <node>   e.g. $0 ssh 2 r1"
    is_ios_lab "$dir" || die "$dir has no SSH-enabled IOS nodes - use: docker exec -it clab-$(lab_name "$dir")-$node bash"
    exec ssh "admin@clab-$(lab_name "$dir")-$node"
}

cmd_graph() {
    local dir; dir="$(resolve_lab "${1:-}")"
    info "topology graph on http://localhost:50080 - Ctrl-C to stop"
    clab graph -t "$(topo_file "$dir")"
}

usage() {
    cat <<'EOF'
deploy.sh - one entry point for the CCNP containerlab study labs.

  ./deploy.sh check                 verify prerequisites and images
  ./deploy.sh list                  list labs and their deployment state
  ./deploy.sh deploy <lab> [opts]   deploy a lab (opts: --solved --reconfigure)
  ./deploy.sh destroy <lab>|all     destroy a lab, keep saved configs (NVRAM)
  ./deploy.sh reset <lab>           destroy AND wipe saved state -> back to baseline
  ./deploy.sh redeploy <lab> [opts] destroy (keep state) + deploy
  ./deploy.sh status [lab]          containerlab inspect for one lab / all labs
  ./deploy.sh save <lab>            save running-config -> startup on every IOS node
  ./deploy.sh ssh <lab> <node>      ssh into an IOS node (admin/admin)
  ./deploy.sh graph <lab>           serve the topology graph on :50080

<lab> may be given as: lab02 | 02 | 2 | lab02-ospf

Environment overrides (also see README.md):
  CCNP_IOL_IMAGE      L3 router image  (default vrnetlab/cisco_iol:17.12.01)
  CCNP_IOL_L2_IMAGE   L2 switch image  (default vrnetlab/cisco_iol:L2-17.12.01)
  CCNP_CFG            configs | solutions (set automatically by --solved)
EOF
}

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
