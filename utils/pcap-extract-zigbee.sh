#!/bin/sh

BASEDIR="$(realpath -- $(dirname $0))"

if [ $# -lt 1 ]; then
    echo "usage: $0 <input.pcap>"
    exit 1
fi

for name in "$@"; do
    tshark -r "${name}" -lq -T ek -x | "${BASEDIR}"/filter-zigbee.jq
done
