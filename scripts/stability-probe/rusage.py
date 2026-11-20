#!/usr/bin/env python3
"""Run a command; print wall time and the children's rusage as key=value pairs."""
import resource, subprocess, sys, time

t0 = time.perf_counter_ns()
rc = subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL).returncode
wall = time.perf_counter_ns() - t0
r = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"wall_ns={wall} maxrss_kb={r.ru_maxrss} minflt={r.ru_minflt} majflt={r.ru_majflt} "
      f"inblock={r.ru_inblock} oublock={r.ru_oublock} nvcsw={r.ru_nvcsw} nivcsw={r.ru_nivcsw} "
      f"utime_us={int(r.ru_utime*1e6)} stime_us={int(r.ru_stime*1e6)}")
sys.exit(rc)
