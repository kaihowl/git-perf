#!/usr/bin/env bash
# Measures git-perf's CI benchmark workloads (add, report) with several
# metrics and writes CSV rows: runner,cpu,workload,rep,metric,value
# Usage: probe.sh <git-perf-binary> <output.csv> <runner-label>
set -uo pipefail
BIN=$(realpath "$1"); OUT=$2; LABEL=$3
HERE=$(cd "$(dirname "$0")" && pwd)
WORK=$(mktemp -d)
CPU=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs | tr -s ' ,' '_')
[ -z "$CPU" ] && CPU=$(lscpu | awk -F: '/Model name/{print $2; exit}' | xargs | tr -s ' ,' '_')
REPS=${REPS:-5}; VG_REPS=${VG_REPS:-3}

emit() { echo "$LABEL,$CPU,$1,$2,$3,$4" >> "$OUT"; }

# Make "git perf" resolve to the binary under test.
mkdir -p "$WORK/bin"; ln -sf "$BIN" "$WORK/bin/git-perf"; export PATH="$WORK/bin:$PATH"

# --- Environment facts -------------------------------------------------------
{
  echo "cpu=$CPU nproc=$(nproc) kernel=$(uname -r)"
  echo "perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null)"
  echo "pmu_devices=$(ls /sys/bus/event_source/devices 2>/dev/null | tr '\n' ' ')"
  echo "cpu_flags_avx512f=$(grep -m1 -c avx512f /proc/cpuinfo)"
  echo "dmi_product=$(cat /sys/class/dmi/id/product_name 2>/dev/null)"
  echo "imds_vmsize=$(curl -s -m 3 -H Metadata:true 'http://169.254.169.254/metadata/instance/compute/vmSize?api-version=2021-02-01&format=text' 2>&1 || echo unreachable)"
  echo "governor=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo none)"
} | tee "$WORK/env.txt"
sed 's/^/# /' "$WORK/env.txt" >> "$OUT.env"

# --- Deterministic fixture (same as CI, but fixed values and dates) ---------
export GIT_AUTHOR_NAME=bench GIT_AUTHOR_EMAIL=bench@example.com
export GIT_COMMITTER_NAME=bench GIT_COMMITTER_EMAIL=bench@example.com
export GIT_AUTHOR_DATE="2024-01-01T00:00:00Z" GIT_COMMITTER_DATE="2024-01-01T00:00:00Z"
export GIT_CONFIG_NOSYSTEM=1 HOME="$WORK/home"; mkdir -p "$HOME"
git config --global maintenance.auto false; git config --global gc.auto 0
BASE="$WORK/base"; git init -q "$BASE"
for i in $(seq 1 10); do
  echo "commit $i" > "$BASE/file$i.txt"; git -C "$BASE" add "file$i.txt"; git -C "$BASE" commit -qm "Commit $i"
  for j in $(seq 1 10); do git -C "$BASE" perf add -m benchmark-a -k iteration="$i-$j" $((100 + (i*7+j*13) % 50)); done
done
for i in $(seq 1 50); do git -C "$BASE" perf add -m inner-test-metric $((100 + i)); done

# Each workload runs in a fresh copy of the fixture so every rep does identical work.
run_workload() { # $1=workload $2..=wrapper
  local w=$1; shift
  rm -rf "$WORK/repo"; cp -a "$BASE" "$WORK/repo"; sync
  case $w in
    add)    "$@" git -C "$WORK/repo" perf add -m inner-test-metric 123 ;;
    report) "$@" git -C "$WORK/repo" perf report -n 10 -o "$WORK/report.html" ;;
  esac
}

PERF=""
for p in perf $(ls /usr/lib/linux-tools/*/perf 2>/dev/null); do
  if command -v "$p" >/dev/null && "$p" --version >/dev/null 2>&1; then PERF=$p; break; fi
done
SUDO=""; [ "$(id -u)" != 0 ] && command -v sudo >/dev/null && SUDO="sudo -E"
EVENTS=instructions:u,instructions,cycles:u,branches:u,cache-references,cache-misses,LLC-load-misses,page-faults,context-switches,task-clock

for w in add report; do
  for rep in $(seq 1 "$REPS"); do
    # wall time + rusage (faults, block I/O, maxrss, ctx switches)
    for kv in $(run_workload $w python3 "$HERE/rusage.py"); do emit $w "$rep" "${kv%%=*}" "${kv#*=}"; done
    # syscall count (I/O work proxy, speed independent)
    run_workload $w strace -f -c -o "$WORK/strace.txt" >/dev/null 2>&1 \
      && emit $w "$rep" syscalls "$(awk '/total$/{print $(NF-2)}' "$WORK/strace.txt")"
    # hardware counters via perf (includes child git processes)
    if [ -n "$PERF" ]; then
      run_workload $w $SUDO "$PERF" stat -x, -e "$EVENTS" -o "$WORK/perf.txt" -- >/dev/null 2>&1
      grep -v '^#' "$WORK/perf.txt" | grep -v '^$' | while IFS=, read -r val _unit ev _rest; do
        case $val in ""|"<not supported>"|"<not counted>") val=NA;; esac
        emit $w "$rep" "perf:$ev" "$val"
      done
    else
      emit $w "$rep" "perf:unavailable" NA
    fi
  done
  # Valgrind: instruction counts + simulated cache misses with a FIXED cache model
  for rep in $(seq 1 "$VG_REPS"); do
    rm -f "$WORK"/vg.*
    run_workload $w valgrind --tool=cachegrind --trace-children=yes --cache-sim=yes \
      --I1=32768,8,64 --D1=32768,8,64 --LL=8388608,16,64 \
      --cachegrind-out-file=/dev/null --log-file="$WORK/vg.%p" >/dev/null 2>&1
    for m in "I *refs" "I1 *misses" "LLi *misses" "D *refs" "D1 *misses" "LLd *misses" "LL misses"; do
      v=$(cat "$WORK"/vg.* | grep -E "== $m:" | awk -F: '{gsub(/[ ,]/,"",$2); s+=$2} END{print s+0}')
      emit $w "$rep" "vg:$(echo "$m" | tr -d ' *')" "$v"
    done
  done
done

# --- A/A interleaved: same binary twice, alternating, ratio of medians -------
cp "$BIN" "$WORK/bin/git-perf-b"
for rep in $(seq 1 15); do
  for v in a b; do
    [ $v = a ] && ln -sf "$BIN" "$WORK/bin/git-perf" || ln -sf "$WORK/bin/git-perf-b" "$WORK/bin/git-perf"
    for kv in $(run_workload report python3 "$HERE/rusage.py"); do
      [ "${kv%%=*}" = wall_ns ] && emit "report-aa-$v" "$rep" wall_ns "${kv#*=}"
    done
  done
done
ln -sf "$BIN" "$WORK/bin/git-perf"
rm -rf "$WORK"
