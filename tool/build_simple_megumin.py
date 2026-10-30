"""Build the flat-color terminal-native mascot (no bitmap dependencies).

Every part is hand-placed pixel text on a 32 x 40 grid, then doubled to the
64 x 80 JSON the TUI embeds. '.' is transparent.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
W, H = 32, 40
PALETTE = {'o':'#30242e','h':'#594047','r':'#ae343d','g':'#d6a258',
           's':'#f2d1b7','b':'#49332b','w':'#e6ddd1','c':'#cc7770',
           'f':'#e8743b','y':'#f7dc7a','k':'#a59a9c'}

HAT = [
    "...........oooo............",
    ".........oohhhhoo..........",
    "........ohhhhhhhho.........",
    "........ohhhhhhhhhoo.......",
    ".......ohhhhhhhhhhhhoo.....",
    ".......ohhhhhhhhhhhhhhoo...",
    "......ohhhhhhhhhhhoooohhoo.",
    "......ohrrrhhhhhrrrho..oogo",
    "......orgrrhhhhrrgrho...ogo",
    ".....ohrrrhhhhhhrrrhho...o.",
    "....ogggggggggggggggggo....",
    "...orfrrfrrfrrfrrfrrfrro...",
    "oogggggggggggggggggggggggoo",
    ".ooooooooooooooooooooooooo.",
]

HEAD = [
    ".obbbbbbbbbbbbbbbbo.",
    "obbbbbsbbbbbbsbbbbbo",
    "obbsssssbssbsssssbbo",
    "obsoooossssssoooosbo",
    "obsrrwrssssssrrwrsbo",
    "obsrrrrssssssrrrrsbo",
    "obbccsssssssssssccbo",
    "obbbsssssoosssssbbbo",
    ".obbbssssssssssbbbo.",
    ".obb.oooooooooo.bbo.",
]
HAIR_TIPS = [
    ".ob..............bo.",
    ".o................o.",
]

# Facial expressions, drawn over HEAD rows 3..7 (columns 3..16).
EYES = {
    'open':   ["oooo......oooo", "rrwr......rrwr", "rrrr......rrrr"],
    'closed': ["..........    ", "oooo......oooo", "..........    "],
    'smug':   [".oo........oo.", "o..o......o..o", ".............."],
    'fierce': ["ooo........ooo", "rrwro....orwrr", ".rrr......rrr."],
    'tired':  ["..............", "oooo......oooo", ".ss........ss."],
    'swirl':  [".oo........oo.", "oyro......oyro", ".oo........oo."],
}
MOUTHS = {
    'smile': ["......oo......", "..............", ""],
    'open':  ["......oo......", "......rr......", "......oo......"],
    'shout': [".....oooo.....", ".....orro.....", "......oo......"],
    'flat':  [".....oooo.....", "..............", ""],
}

CAPE = [
    ".....oooooooooooooooo.....",
    "....oooooooooooooooooo....",
    "....oooooooooooooooooo....",
    "...oooooooooooooooooooo...",
    "...oooooooooooooooooooo...",
    "..oooooooooooooooooooooo..",
    "..oooooooooooooooooooooo..",
    ".oooooooooooooooooooooooo.",
    ".oooooooooooooooooooooooo.",
    "oooooooooooooooooooooooooo",
    "ogoooooooooooooooooooooogo",
    "gg.ooooooooooooooooooo..gg",
]

DRESS = [
    "..rrrrorrrr..",
    ".rrrrrorrrrr.",
    ".rrrrogorrrr.",
    ".rrrrrorrrrr.",
    ".ooooowoooooo",
    ".oooowowooooo",
    "rrrrrrrrrrrrr",
    "rrrrrrrrrrrrrr",
    "gfgfgfgfgfgfgf",
]

LEGS = [
    "..ssss...oooo..",
    "..wwww...oooo..",
    "..ssss...oooo..",
    ".obbbbo.obbbbo.",
    ".ogggbo.ogggbo.",
]

STAFF_HEAD = [
    "..ooooo.",
    ".obbbbbo",
    "obooo.bo",
    "obryo.bo",
    "obrro.bo",
    ".ooo.obo",
    ".....obo",
]


class Canvas:
    def __init__(self):
        self.p = [['.'] * W for _ in range(H)]

    def put(self, x, y, c):
        if 0 <= x < W and 0 <= y < H:
            self.p[y][x] = c

    def stamp(self, art, x, y, flip=False):
        for dy, row in enumerate(art):
            row = row[::-1] if flip else row
            for dx, c in enumerate(row):
                if c not in '. ':
                    self.put(x + dx, y + dy, c)

    def rect(self, x, y, w, h, c):
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.put(xx, yy, c)

    def line(self, x, y, xx, yy, c, thick=1):
        steps = max(abs(xx - x), abs(yy - y), 1)
        for i in range(steps + 1):
            px = round(x + (xx - x) * i / steps)
            py = round(y + (yy - y) * i / steps)
            self.rect(px, py, thick, 1, c)

    def rows(self):
        return [''.join(r) for r in self.p]


def staff(cv, x, top, bottom):
    """Vertical staff whose shaft is column x (outline at x-1 and x+1)."""
    cv.stamp(STAFF_HEAD, x - 6, top)
    for y in range(top + 7, bottom + 1):
        cv.put(x - 1, y, 'o'); cv.put(x, y, 'b'); cv.put(x + 1, y, 'o')
    cv.put(x, bottom, 'o')


def head(cv, x, y, eyes='open', mouth='smile', blush=True):
    cv.stamp(HAT, x - 4, y - 14)
    cv.stamp(HEAD, x, y)
    for i, row in enumerate(EYES[eyes]):
        for dx, c in enumerate(row):
            cv.put(x + 3 + dx, y + 3 + i, 's' if c in '. ' else c)
    for i, row in enumerate(MOUTHS[mouth]):
        for dx, c in enumerate(row):
            if c not in '. ':
                cv.put(x + 3 + dx, y + 7 + i, c)
    if not blush:
        cv.rect(x + 3, y + 6, 2, 1, 's'); cv.rect(x + 15, y + 6, 2, 1, 's')


def body(cv, x, y, cape_flare=0):
    cape = [row for row in CAPE]
    if cape_flare:
        cape = [('.' * 0) + row for row in cape]
    cv.stamp(cape, x - 3, y + 1)
    cv.stamp(DRESS, x + 3, y)
    cv.stamp(LEGS, x + 2, y + 9)


def standing(eyes='open', mouth='smile', bob=0, arm='side', hold='staff'):
    cv = Canvas()
    hx, hy = 4, 14 + bob
    body(cv, hx, 24 + bob)
    # Legs stay planted when the upper body breathes down one pixel.
    cv.rect(0, 33 + bob, W, 7, '.')
    cv.stamp(CAPE[-2:], hx - 3, 33 + bob)
    cv.stamp(LEGS, hx + 2, 34)
    # Left arm (viewer's left).
    if arm == 'side':
        cv.stamp(["orro", "orro", "orro", "orro", ".oss", ".oss"], hx + 1, 25 + bob)
    elif arm == 'point':
        cv.stamp(["...ooo", ".ooorr", "osssrr", "ossoo."], hx - 3, 25 + bob)
        for sx, sy in ((0, 21), (1, 23), (0, 26)):
            cv.put(sx, sy + bob, 'y')
    elif arm == 'hip':
        cv.stamp(["..orr", ".orro", "orro.", "osso.", ".oss."], hx + 1, 25 + bob)
    head(cv, hx, hy, eyes, mouth)
    cv.stamp(HAIR_TIPS, hx, hy + 10)
    if hold == 'staff':
        staff(cv, 27, 12 + bob, 39)
        cv.stamp(["orrro.", "orrrss", ".ooooss"], hx + 15, 26 + bob)
    return cv


def chant(phase):
    """Staff raised high, both eyes fierce, sparks circling the orb."""
    cv = Canvas()
    hx, hy = 3, 15
    body(cv, hx, 25)
    cv.stamp(["orro", "orro", "orro", "orro", ".oss", ".oss"], hx + 1, 26)
    head(cv, hx, hy, 'fierce', 'open' if phase else 'shout')
    cv.stamp(HAIR_TIPS, hx, hy + 10)
    staff(cv, 26, 1, 33)
    # Raised arm reaching up the staff.
    cv.stamp(["..oss", ".orss", "orro.", "orro.", "orro."], 22, 13)
    sparks = [(29, 0), (31, 4), (20, 2), (29, 9)] if phase else [(31, 1), (19, 0), (31, 8), (21, 6)]
    for i, (sx, sy) in enumerate(sparks):
        cv.put(sx, sy, 'y' if i % 2 else 'f')
        if phase:
            cv.put(sx, sy + 1, 'r')
    cv.put(24, 4, 'y' if phase else 'f')  # glowing orb
    return cv


def cast_pose(cv, eyes='fierce', mouth='shout'):
    hx, hy = 1, 15
    body(cv, hx, 25)
    cv.stamp(["orro", "orro", "orro", "orro", ".oss", ".oss"], hx + 1, 26)
    head(cv, hx, hy, eyes, mouth)
    cv.stamp(HAIR_TIPS, hx, hy + 10)
    # Staff thrust forward on a diagonal, orb pointing up-right.
    cv.line(17, 33, 25, 19, 'o', 2)
    cv.line(18, 32, 25, 20, 'b')
    cv.stamp([".ooo.", "obrbo", "orybo", ".ooo."], 24, 16)
    cv.stamp(["orrrss", "orrrss", ".oooo."], 15, 27)


def ring(cv, cx, cy, r, c, thick=1.0):
    for y in range(H):
        for x in range(W):
            d = ((x - cx) ** 2 + ((y - cy) * 1.0) ** 2) ** 0.5
            if r - thick <= d <= r:
                cv.put(x, y, c)


def disk(cv, cx, cy, r, c):
    for y in range(H):
        for x in range(W):
            if (x - cx) ** 2 + (y - cy) ** 2 <= r * r:
                cv.put(x, y, c)


def boom(stage):
    cv = Canvas()
    if stage == 1:
        cast_pose(cv)
        ring(cv, 27, 13, 4.5, 'f', 1.2)
        for x, y in ((27, 7), (31, 13), (23, 9), (30, 9)):
            cv.put(x, y, 'y')
    elif stage == 2:
        disk(cv, 25, 13, 7.5, 'r')
        disk(cv, 25, 13, 6, 'f')
        disk(cv, 25, 13, 4, 'y')
        disk(cv, 25, 13, 2, 'w')
        for x, y in ((16, 3), (31, 1), (18, 22), (31, 24), (14, 9), (25, 0)):
            cv.put(x, y, 'y'); cv.put(x, y + 1, 'f')
        cast_pose(cv, 'fierce', 'shout')
    else:
        for cx, cy, r in ((24, 10, 5), (29, 15, 4), (20, 14, 3.5), (27, 5, 3.5)):
            disk(cv, cx, cy, r, 'k')
        for cx, cy, r in ((23, 9, 2.5), (28, 14, 2)):
            disk(cv, cx, cy, r, 'w')
        for x, y in ((17, 4), (31, 21), (15, 18), (30, 0)):
            cv.put(x, y, 'f')
        cast_pose(cv, 'swirl', 'open')
    return cv


def down():
    """Collapsed face-down after the spell: hat tipped forward, staff dropped."""
    cv = Canvas()
    # Staff lying flat behind her.
    cv.rect(2, 37, 29, 1, 'o'); cv.rect(3, 38, 27, 1, 'b'); cv.rect(2, 39, 29, 1, 'o')
    cv.stamp([".ooo.", "obrbo", ".ooo."], 27, 36)
    # Cape and body lying to the right.
    cv.stamp([
        "....oooooooooooo....",
        "..oorrrrrrrrrrrooo..",
        ".oorrrrrrorrrrrrooo.",
        "oooooowooooooooooggo",
        ".ooooooooooooooooooo",
    ], 11, 31)
    cv.stamp(["ssww", "obbo", "oggo"], 28, 33)
    cv.stamp([".oss", "osso"], 8, 34)
    # Hat fallen over the head, eyes peeking under the brim.
    hat = [row[:] for row in HAT[4:]]
    cv.stamp(hat, 0, 21)
    cv.stamp([
        "obbbbbbbbbbbbbbbbo",
        "obsoooossssoooosbo",
        "obbccsssoosssccbbo",
        ".oooooooooooooooo.",
    ], 2, 31)
    # Exhaustion puff.
    cv.stamp([".kk.", "kwwk", ".kk."], 26, 22)
    cv.put(25, 25, 'k'); cv.put(24, 27, 'k')
    return cv


def main():
    idle = standing().rows()
    frames = {
        'idle': idle,
        'idle_breath': standing(bob=1).rows(),
        'blink': standing('closed').rows(),
        'pose': standing('open', 'open', arm='point').rows(),
        'pose_closed': standing('smug', 'smile', arm='point').rows(),
        'chant1': chant(0).rows(),
        'chant2': chant(1).rows(),
        'boom1': boom(1).rows(),
        'boom2': boom(2).rows(),
        'boom3': boom(3).rows(),
        'down': down().rows(),
    }
    encode = lambda rows: [''.join(c * 2 for c in row) for row in rows for _ in range(2)]
    path = ROOT / 'design/tui/pet/megumin.json'
    # Behaviour lives in the pack too, so other pets can define their own.
    chanting = [['chant1', 200], ['chant2', 200]] * 2
    behaviour = {
        'name': '惠惠',
        'idle': {'frames': [['idle', 900], ['idle_breath', 900]], 'blink': 'blink'},
        'clicks': ['intro', 'explosion'],
        'actions': [
            {'id': 'intro', 'frames': [['pose', 240], ['pose_closed', 240]], 'duration': 1600,
             'quotes': [[0, '「吾名惠惠！」']]},
            {'id': 'explosion',
             'frames': chanting + [['boom1', 120], ['boom2', 450], ['boom3', 600]],
             'next': 'exhausted', 'reduced': 'exhausted',
             'quotes': [[0, '「比黑更黑……」'], [800, '「Explosion！」']]},
            {'id': 'exhausted', 'frames': [['down', 0]], 'hold': True,
             'quotes': [[0, '「……动不了了。」']], 'release_quote': '「明天还要再放一发。」'},
        ],
    }
    path.write_text(json.dumps({**behaviour, 'width': 64, 'height': 80,
                                'style': 'flat native pixel art', 'palette': PALETTE,
                                'frames': {k: encode(v) for k, v in frames.items()}},
                               ensure_ascii=False, indent=1) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
