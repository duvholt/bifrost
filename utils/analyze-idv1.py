#!/usr/bin/env python3

import sys
import json
import re
from rich import print

RE_NUM = re.compile("/[0-9]+$")

def load(name):
    # print(f"loading {name}..")
    return json.load(open(name))["data"]

def idv1_type(idv1):
    if not idv1.endswith("/0"):
        idv1 = RE_NUM.sub("/:id", idv1)
    if idv1.startswith("/scenes/"):
        idv1 = "/scenes/:id"
    return idv1


def analyze_unique_ids(res):
    ids = set()

    for obj in res:
        if "id_v1" in obj:
            rtype = obj["type"]
            idv1 = idv1_type(obj["id_v1"])
            ids.add((idv1, rtype))

    for idv1, rtype in sorted(ids):
        print(f"{idv1:20} {rtype:30}")


def analyze_idv1_percent(res):
    ids = dict()

    for obj in res:
        rtype = obj["type"]
        if rtype not in ids:
            ids[rtype] = [0, 0]

        cnt = ids[rtype]
        cnt[0] += 1
        if "id_v1" in obj:
            cnt[1] += 1

    for name, (total, idv1) in sorted(ids.items()):
        pct = (idv1 / total) * 100.0
        print(f"{name:30} {pct:6.2f}% [{idv1:5} / {total:5}]")


def analyze_idv1_mapping(res):
    ids = dict()

    for obj in res:
        rtype = obj["type"]
        if rtype not in ids:
            ids[rtype] = set()

        if "id_v1" in obj:
            idv1 = idv1_type(obj["id_v1"])
        else:
            idv1 = "<none>"

        ids[rtype].add(idv1)

    for name, types in sorted(ids.items()):
        if types == {"<none>"}:
            continue
        print(f"{name:30} {'   '.join(sorted(types, reverse=True))}")


def main(args):
    objs = []
    for name in args:
        objs.extend(load(name))

    analyze_unique_ids(objs)
    analyze_idv1_percent(objs)
    analyze_idv1_mapping(objs)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
