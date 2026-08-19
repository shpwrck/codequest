#!/usr/bin/env /usr/bin/python3
"""Segment a GBA face image (reference art or app screenshot) into components
and report each bbox in body-relative fractions, so two images can be compared
regardless of scale."""
import sys, json
from collections import deque
from PIL import Image

def load(path):
    im = Image.open(path).convert('RGB')
    return im, im.load()

def measure(path, kind):
    im, px = load(path)
    W, H = im.size

    def is_bg_seed(p):
        r, g, b = p
        if kind == 'ref':
            return (r > 110 and r - g > 50 and r - b > 50) or (r > 200 and g > 200 and b > 200)
        return max(p) < 60

    # flood background from corners/edges
    bg = bytearray(W * H)
    dq = deque()
    for x in range(W):
        for y in (0, H - 1):
            if is_bg_seed(px[x, y]) and not bg[y * W + x]:
                bg[y * W + x] = 1; dq.append((x, y))
    for y in range(H):
        for x in (0, W - 1):
            if is_bg_seed(px[x, y]) and not bg[y * W + x]:
                bg[y * W + x] = 1; dq.append((x, y))
    while dq:
        x, y = dq.popleft()
        for nx, ny in ((x+1,y),(x-1,y),(x,y+1),(x,y-1)):
            if 0 <= nx < W and 0 <= ny < H and not bg[ny * W + nx] and is_bg_seed(px[nx, ny]):
                bg[ny * W + nx] = 1; dq.append((nx, ny))

    def cls(p):
        r, g, b = p
        mx, mn = max(r, g, b), min(r, g, b)
        if g > 110 and g - r > 40 and g - b > 40: return 'led'
        if mn > 95 and mx - mn < 45 and mx > 135: return 'gray'
        if mx < 78: return 'dark'
        if b - max(r, g) > 25 and b > 95: return 'body'
        return 'other'

    # body bbox from indigo pixels
    bx0 = by0 = 10**9; bx1 = by1 = -1
    labels = {}
    for y in range(H):
        for x in range(W):
            if bg[y * W + x]: continue
            c = cls(px[x, y])
            if c == 'body':
                if x < bx0: bx0 = x
                if x > bx1: bx1 = x
                if y < by0: by0 = y
                if y > by1: by1 = y
    bw, bh = bx1 - bx0 + 1, by1 - by0 + 1

    # connected components for gray / dark / led
    seen = bytearray(W * H)
    blobs = []
    for y0 in range(H):
        for x0 in range(W):
            i0 = y0 * W + x0
            if seen[i0] or bg[i0]: continue
            c0 = cls(px[x0, y0])
            if c0 not in ('gray', 'dark', 'led'): seen[i0] = 1; continue
            # BFS
            seen[i0] = 1
            q = deque([(x0, y0)])
            n = 0; mnx = mxx = x0; mny = mxy = y0
            while q:
                x, y = q.popleft(); n += 1
                if x < mnx: mnx = x
                if x > mxx: mxx = x
                if y < mny: mny = y
                if y > mxy: mxy = y
                for nx, ny in ((x+1,y),(x-1,y),(x,y+1),(x,y-1)):
                    j = ny * W + nx
                    if 0 <= nx < W and 0 <= ny < H and not seen[j] and not bg[j] and cls(px[nx, ny]) == c0:
                        seen[j] = 1; q.append((nx, ny))
            if n > 60:
                blobs.append({'cls': c0, 'n': n, 'box': (mnx, mny, mxx, mxy)})

    def frac(box):
        x0, y0, x1, y1 = box
        return {'cx': round((x0 + x1) / 2 - bx0, 1) / bw, 'cy': round((y0 + y1) / 2 - by0, 1) / bh,
                'w': (x1 - x0 + 1) / bw, 'h': (y1 - y0 + 1) / bh}

    out = {'size': [W, H], 'body': [bx0, by0, bw, bh], 'aspect': round(bw / bh, 3), 'components': {}}
    G = [b for b in blobs if b['cls'] == 'gray']
    D = [b for b in blobs if b['cls'] == 'dark']
    L = [b for b in blobs if b['cls'] == 'led']

    def fx(b): return ((b['box'][0] + b['box'][2]) / 2 - bx0) / bw
    def fy(b): return ((b['box'][1] + b['box'][3]) / 2 - by0) / bh

    # lens: largest dark blob near center
    lens = max((b for b in D if 0.25 < fx(b) < 0.75), key=lambda b: b['n'], default=None)
    if lens: out['components']['lens'] = frac(lens['box'])

    # lcd: largest gray blob fully inside lens (reference art only, usually)
    if lens:
        lx0, ly0, lx1, ly1 = lens['box']
        lcd = max((b for b in G if b['box'][0] > lx0 and b['box'][2] < lx1
                   and b['box'][1] > ly0 and b['box'][3] < ly1), key=lambda b: b['n'], default=None)
        if lcd:
            out['components']['lcd'] = frac(lcd['box'])
            G = [b for b in G if b is not lcd]

    # shoulders: gray blobs whose top touches the top strip
    for b in sorted(G, key=lambda b: -b['n']):
        if (b['box'][1] - by0) / bh < 0.04 and b['n'] > 400:
            name = 'shoulder_l' if fx(b) < 0.5 else 'shoulder_r'
            if name not in out['components']: out['components'][name] = frac(b['box'])
    G = [b for b in G if ((b['box'][1] - by0) / bh) >= 0.04 or b['n'] <= 400]

    # dpad: largest gray on left-middle
    dpad = max((b for b in G if fx(b) < 0.35 and 0.2 < fy(b) < 0.8), key=lambda b: b['n'], default=None)
    if dpad:
        out['components']['dpad'] = frac(dpad['box']); G = [b for b in G if b is not dpad]

    # A and B on the right: round-ish blobs only, two largest; rightmost = A
    def roundish(b):
        x0, y0, x1, y1 = b['box']
        w, h = x1 - x0 + 1, y1 - y0 + 1
        return 0.7 < w / h < 1.4
    ab = sorted((b for b in G if fx(b) > 0.6 and fy(b) < 0.8 and b['n'] > 300 and roundish(b)),
                key=lambda b: -b['n'])[:2]
    ab.sort(key=lambda b: -fx(b))
    if ab: out['components']['btn_a'] = frac(ab[0]['box'])
    if len(ab) > 1: out['components']['btn_b'] = frac(ab[1]['box'])

    # start/select: bottom-left small grays, two largest, upper = start
    ss = sorted((b for b in G if fx(b) < 0.32 and fy(b) > 0.55 and b['n'] > 150), key=lambda b: -b['n'])[:2]
    ss.sort(key=lambda b: fy(b))
    if ss: out['components']['btn_start'] = frac(ss[0]['box'])
    if len(ss) > 1: out['components']['btn_select'] = frac(ss[1]['box'])

    # led
    if L: out['components']['led'] = frac(max(L, key=lambda b: b['n'])['box'])

    # speaker: aggregate small dark blobs bottom-right, not overlapping gray boxes
    def overlaps_gray(b):
        x0, y0, x1, y1 = b['box']
        for g in G + ab + ss:
            gx0, gy0, gx1, gy1 = g['box']
            if x0 <= gx1 + 4 and x1 >= gx0 - 4 and y0 <= gy1 + 4 and y1 >= gy0 - 4: return True
        return False
    sp = [b for b in D if b is not lens and fx(b) > 0.7 and 0.45 < fy(b) < 0.95
          and b['n'] < 20000 and not overlaps_gray(b)]
    if sp:
        x0 = min(b['box'][0] for b in sp); y0 = min(b['box'][1] for b in sp)
        x1 = max(b['box'][2] for b in sp); y1 = max(b['box'][3] for b in sp)
        out['components']['speaker'] = frac((x0, y0, x1, y1))

    return out

if __name__ == '__main__':
    path, kind = sys.argv[1], sys.argv[2]  # kind: ref | shot
    print(json.dumps(measure(path, kind), indent=1))
