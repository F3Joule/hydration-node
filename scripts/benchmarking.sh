#!/usr/bin/env bash
# Created by Moonbeam/Purestake Developers. Shamelessly copied from Moonbeam's benchmarking script
# Original repository: https://github.com/moonbeam-foundation/moonbeam

# This script can be used for running HydraDX's benchmarks.
#
# The hydradx binary is required to be compiled with --features=runtime-benchmarks
# in release mode.

set -e

BINARY="./target/release/hydradx"
STEPS=50
REPEAT=20

function help {
    echo "USAGE:"
    echo "  ${0} [<pallet> <benchmark>] [--check] [--all] [--bin <path>]"
    echo ""
    echo "EXAMPLES:"
    echo "  ${0}                       " "list all benchmarks and provide a selection to choose from"
    echo "  ${0} --check               " "list all benchmarks and provide a selection to choose from, runs in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} foo bar               " "run a benchmark for pallet 'foo' and benchmark 'bar'"
    echo "  ${0} foo bar --check       " "run a benchmark for pallet 'foo' and benchmark 'bar' in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} --all         " "run a benchmark for all pallets"
    echo "  ${0} --all --check " "run a benchmark for all pallets in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} --bin <path>  " "specify a path to the benchmark binary"
}

function choose_and_bench {
    while read benchmark; do
        options+=("$benchmark")
    done < <(${BINARY} benchmark pallet --list | sed 1d)

    options+=('EXIT')

    select opt in "${options[@]}"; do
        IFS=', ' read -ra parts <<< "${opt}"
        [[ "${opt}" == 'EXIT' ]] && exit 0

        bench "${parts[0]}" "${parts[1]}" "${1}"
        break
    done
}

function bench {
    if [[ ! -f "${BINARY}" ]]; then
        echo "binary '${BINARY}' does not exist."
        echo "ensure that the hydradx binary is compiled with '--features=runtime-benchmarks' and in release mode."
        exit 1
    fi

    OUTPUT=${4:-weights.rs}
    echo "benchmarking '${1}::${2}' --check=${3}, writing results to '${OUTPUT}'"

    # Check enabled
    if [[ "${3}" -eq 1 ]]; then
        STEPS=16
        REPEAT=1
    fi

    WASMTIME_BACKTRACE_DETAILS=1 ${BINARY} benchmark pallet \
        --wasm-execution=compiled \
        --pallet "${1}" \
        --extrinsic "${2}" \
        --heap-pages 4096 \
        --steps "${STEPS}" \
        --repeat "${REPEAT}" \
        --template=scripts/pallet-weight-template.hbs \
        --output "${OUTPUT}"
}

CHECK=0
ALL=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin)
            shift
            BINARY="$1"
            ;;
        --check)
            CHECK=1
            ;;
        --all)
            ALL=1
            ;;
        --help)
            help
            exit 0
            ;;
        *)
            ARGS+=("$1")
            ;;
    esac
    shift
done

if [[ "${ALL}" -eq 1 ]]; then
    mkdir -p weights/
    bench '*' '*' "${CHECK}" "weights/"
elif [[ ${#ARGS[@]} -ne 2 ]]; then
    choose_and_bench "${CHECK}"
else
    bench "${ARGS[0]}" "${ARGS[1]}" "${CHECK}"
fi
