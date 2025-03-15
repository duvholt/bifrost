#!/usr/bin/env -S jq -c --unbuffered -f

### convert json file:
#
# tshark -r zigbee-2025-01-27_08.57.08-hue.pcap  -lq -T ek -x | utils/pcap-extract-zigbee.jq > /tmp/sample.json
#

### live capture
#
# tshark -i lo -lq -T ek -x 'udp port 1337' | utils/pcap-extract-zigbee.jq
#

.layers
  | select(.zbee_aps["zbee_aps_zbee_aps_cluster_raw"])
  | {
    time: .frame["frame_frame_time_epoch"],
    index: .frame["frame_frame_number"] | tonumber,
    src_mac: (.zbee_nwk["zbee_nwk_zbee_sec_src64_raw"] // .wpan["wpan_wpan_addr64_raw"]),
    src: (.zbee_nwk["zbee_nwk_zbee_nwk_src_raw"] // .wpan["wpan_wpan_addr16_raw"]),
    dst: (.zbee_nwk["zbee_nwk_zbee_nwk_dst_raw"] // .wpan["wpan_wpan_dst16_raw"]),
    cluster: .zbee_aps["zbee_aps_zbee_aps_cluster_raw"],
    cmd: .zbee_zcl["zbee_zcl_zbee_zcl_cmd_id_raw"],
    data: .zbee_zcl_raw
  }
