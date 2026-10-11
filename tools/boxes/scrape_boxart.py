"""Relevé LaunchBox (une fois) : dos et tranche des boîtes des jeux Pokémon pris en charge.

Sortie : box_art.json, jeu -> {"back": ["fr <fichier>", "en <fichier>", ...], "spine": [...]}
Régions gardées : France (fr), puis Europe, Royaume-Uni, Océanie, Australie, Amérique du Nord,
Canada, Monde (en). Jamais une autre langue.
"""
import html, json, re, sys, time, urllib.parse, urllib.request

GAMES = [
    ("red", "Pokemon Red Version", "Nintendo Game Boy"), ("blue", "Pokemon Blue Version", "Nintendo Game Boy"),
    ("yellow", "Pokemon Yellow Version", "Nintendo Game Boy"), ("gold", "Pokemon Gold Version", "Nintendo Game Boy Color"),
    ("silver", "Pokemon Silver Version", "Nintendo Game Boy Color"), ("crystal", "Pokemon Crystal Version", "Nintendo Game Boy Color"),
    ("ruby", "Pokemon Ruby Version", "Nintendo Game Boy Advance"), ("ruby", "Pokemon Ruby", "Nintendo Game Boy Advance"), ("sapphire", "Pokemon Sapphire Version", "Nintendo Game Boy Advance"),
    ("emerald", "Pokemon Emerald Version", "Nintendo Game Boy Advance"), ("fire_red", "Pokemon FireRed Version", "Nintendo Game Boy Advance"),
    ("leaf_green", "Pokemon LeafGreen Version", "Nintendo Game Boy Advance"),
    ("diamond", "Pokemon Diamond Version", "Nintendo DS"), ("pearl", "Pokemon Pearl Version", "Nintendo DS"),
    ("platinum", "Pokemon Platinum Version", "Nintendo DS"), ("heart_gold", "Pokemon HeartGold Version", "Nintendo DS"),
    ("soul_silver", "Pokemon SoulSilver Version", "Nintendo DS"), ("black", "Pokemon Black Version", "Nintendo DS"),
    ("white", "Pokemon White Version", "Nintendo DS"), ("black2", "Pokemon Black Version 2", "Nintendo DS"),
    ("white2", "Pokemon White Version 2", "Nintendo DS"),
    ("x", "Pokemon X", "Nintendo 3DS"), ("y", "Pokemon Y", "Nintendo 3DS"),
    ("omega_ruby", "Pokemon Omega Ruby", "Nintendo 3DS"), ("alpha_sapphire", "Pokemon Alpha Sapphire", "Nintendo 3DS"),
    ("sun", "Pokemon Sun", "Nintendo 3DS"), ("moon", "Pokemon Moon", "Nintendo 3DS"),
    ("ultra_sun", "Pokemon Ultra Sun", "Nintendo 3DS"), ("ultra_moon", "Pokemon Ultra Moon", "Nintendo 3DS"),
    ("nx_0100abf008968000", "Pokemon Sword", "Nintendo Switch"), ("nx_01008db008c2c000", "Pokemon Shield", "Nintendo Switch"),
    ("nx_0100000011d90000", "Pokemon Brilliant Diamond", "Nintendo Switch"), ("nx_010018e011d92000", "Pokemon Shining Pearl", "Nintendo Switch"),
    ("nx_01001f5010dfa000", "Pokemon Legends: Arceus", "Nintendo Switch"), ("nx_0100a3d008c5c000", "Pokemon Scarlet", "Nintendo Switch"),
    ("nx_01008f6008c5e000", "Pokemon Violet", "Nintendo Switch"), ("nx_0100f43008c44000", "Pokemon Legends Z-A", "Nintendo Switch"),
    ("nx_010003f003a34000", "Pokemon: Let's Go Pikachu!", "Nintendo Switch"), ("nx_0100187003a36000", "Pokemon: Let's Go, Eevee!", "Nintendo Switch"),
]
FR = {"France"}
# « Europe » passe après : ses dos sont souvent bilingues (anglais et néerlandais, etc.).
EN = ["United Kingdom", "Oceania", "Australia", "North America", "United States", "Canada", "Europe", "World"]
BASE = "https://gamesdb.launchbox-app.com"


def get(url):
    for _ in range(3):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
            return urllib.request.urlopen(req, timeout=30).read().decode("utf-8", "replace")
        except Exception as e:
            err = e
            time.sleep(2)
    raise err


def norm(t):
    t = html.unescape(t).lower().replace("é", "e").replace("é", "e")
    return re.sub(r"[^a-z0-9]", "", t)


def find_id(query, platform):
    page = get(f"{BASE}/games/results/{urllib.parse.quote(query)}")
    links = list(dict.fromkeys(re.findall(r'href="/games/details/(\d+)-[^"]*"', page)))[:12]
    for gid in links:
        d = get(f"{BASE}/games/details/{gid}")
        title = re.search(r"<h1[^>]*>(.*?)</h1>", d, re.S)
        title = re.sub(r"<[^>]+>", "", title.group(1)).strip() if title else ""
        i = d.find("Platform")
        words = []
        for w in re.sub(r"<[^>]+>", " ", d[i : i + 400]).split()[1:] if i >= 0 else []:
            if w.startswith("<"):
                break
            words.append(w)
        plat = " ".join(words)
        if norm(title) == norm(query) and plat == platform:
            return gid, title
    return None, None


def images(gid):
    page = get(f"{BASE}/games/images/{gid}")
    out = {}
    for url, title in re.findall(r'href="(https://images\.launchbox-app\.com/[^"]+)"[^>]*?data-title="([^"]+)"', page, re.S):
        t = html.unescape(title)
        m = re.search(r" - (Box - (?:Back|Spine|Front)) Image(?: \(([^)]+)\))?$", t)
        if not m:
            continue
        kind = m.group(1).split(" - ")[1].lower()
        out.setdefault(kind, []).append((m.group(2) or "", url.rsplit("/", 1)[1]))
    return out


def ranked(entries):
    res = [f"fr {f}" for r, f in entries if r in FR]
    for region in EN:
        res += [f"en {f}" for r, f in entries if r == region]
    return res


result = {}
for key, query, platform in GAMES:
    if key in result:
        continue
    gid, title = find_id(query, platform)
    if not gid:
        print(f"{key}: introuvable", file=sys.stderr)
        continue
    imgs = images(gid)
    # Face avant : seulement pour la Switch (ailleurs, les boîtes GameTDB et libretro suffisent).
    entry = {k: ranked(v)[:3] for k, v in imgs.items() if ranked(v) and (k != "front" or key.startswith("nx_"))}
    print(f"{key}: {gid} {title} -> " + ", ".join(f"{k} {len(v)}" for k, v in entry.items()), file=sys.stderr)
    if entry:
        result[key] = entry
json.dump(result, open(sys.argv[1], "w", encoding="utf-8", newline="\n"), ensure_ascii=False, indent=1)
