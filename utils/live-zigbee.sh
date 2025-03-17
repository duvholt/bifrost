#!/bin/sh

# this script expects a zigbee sniffer to be running,
# which dumps zigbee packets to udp on localhost
#
# one example is zsmartsystems zigbee sniffer:
#
#   git clone https://github.com/zsmartsystems/com.zsmartsystems.zigbee.sniffer.git
#   cd com.zsmartsystems.zigbee.sniffer
#
#   java -jar com.zsmartsystems.zigbee.sniffer-1.0.2.jar -p /dev/serial/by-id/${ZIGBEE_ADAPTER} -c $ZIGBEE_CHANNEL -b 115200 -flow software -r 1337
#

BASEDIR="$(realpath -- $(dirname $0))"

tshark -ni lo -lq -T ek -x | stdbuf -i0 -o0 ${BASEDIR}/filter-zigbee.jq | cargo run --example=ez-parse
