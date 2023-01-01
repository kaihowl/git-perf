#!/usr/bin/env python3

from datetime import datetime, timezone
from random import randint

with open('test.txt', 'w') as f:
    for metric_id in range(0, 100):
        for measurement_id in range(0, 10000):
            now = datetime.now(timezone.utc).timestamp()
            rand = randint(0, 10000)
            f.write(f"metric_{metric_id} {now} {rand} os=ubuntu-20.04\n")
