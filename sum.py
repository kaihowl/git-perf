s = 0.0
c = 0.0
for i in range(0, 100):
    y = 0.1 - c
    t = s + y
    c = (t - s) - y
    s = t

print(f"naive {sum([0.1]*100) / 100}")
print(f"precise: {s / 100}")
