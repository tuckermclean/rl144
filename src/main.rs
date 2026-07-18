// rl144 — a roguelike in under 1.44MB. Zero asset files; everything procedural or const.
use font8x8::legacy::BASIC_LEGACY;
use minifb::{Key, KeyRepeat, Window, WindowOptions};

const COLS: usize = 80;
const ROWS: usize = 30;
const MAP_H: usize = 25; // rows [0,25) map; row 25 status; rows [26,30) log
const CW: usize = 8;
const CH: usize = 12;
const WIDTH: usize = COLS * CW; // 640
const HEIGHT: usize = ROWS * CH; // 360
const MAX_DEPTH: u32 = 5;
const FOV_R: i32 = 8;

// ---------- RNG (xorshift64) ----------
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// random in [lo, hi)
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % ((hi - lo) as u64)) as i32
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next() % den < num
    }
}

// ---------- Channel RNG ----------
// FNV-1a(64) over the seed bytes plus unit-separated tags, then a splitmix-style
// finalizer. Named channels isolate random streams: combat rolls and AI wander
// can never perturb worldgen, so a seed always generates the same world.
// This hash is a public API once golden fixtures exist: changing any constant
// or the tag scheme invalidates every seed in the wild (MAJOR version bump).
fn h64(seed: u64, tags: &[&str]) -> u64 {
    const PRIME: u64 = 0x100_0000_01b3;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.to_le_bytes() {
        h = (h ^ b as u64).wrapping_mul(PRIME);
    }
    for t in tags {
        h = (h ^ 0x1f).wrapping_mul(PRIME); // unit separator, golem-style
        for b in t.bytes() {
            h = (h ^ b as u64).wrapping_mul(PRIME);
        }
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    h
}

fn channel(seed: u64, tags: &[&str]) -> Rng {
    Rng::new(h64(seed, tags))
}

// ---------- Map ----------
#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Wall,
    Floor,
    Stairs,
}

#[derive(Clone, Copy, PartialEq)]
enum MKind {
    Rat,
    Goblin,
    Ogre,
}

struct Monster {
    x: i32,
    y: i32,
    kind: MKind,
    hp: i32,
}

impl Monster {
    fn stats(kind: MKind) -> (i32, i32, u8, u32, &'static str) {
        // (hp, atk, glyph, color, name)
        match kind {
            MKind::Rat => (3, 1, b'r', 0xB0703A, "rat"),
            MKind::Goblin => (6, 2, b'g', 0x40C040, "goblin"),
            MKind::Ogre => (13, 4, b'O', 0xD05050, "ogre"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum IKind {
    Potion,
    Sword,
    Amulet,
}

struct Item {
    x: i32,
    y: i32,
    kind: IKind,
}

struct Game {
    map: Vec<Tile>,
    seen: Vec<bool>,
    vis: Vec<bool>,
    px: i32,
    py: i32,
    hp: i32,
    maxhp: i32,
    atk: i32,
    depth: u32,
    kills: u32,
    monsters: Vec<Monster>,
    items: Vec<Item>,
    msgs: Vec<String>,
    seed: u64,
    combat_rng: Rng,
    ai_rng: Rng,
    dead: bool,
    won: bool,
}

fn idx(x: i32, y: i32) -> usize {
    y as usize * COLS + x as usize
}
fn in_map(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < COLS as i32 && y < MAP_H as i32
}

impl Game {
    fn new(seed: u64) -> Self {
        let mut g = Game {
            map: vec![Tile::Wall; COLS * MAP_H],
            seen: vec![false; COLS * MAP_H],
            vis: vec![false; COLS * MAP_H],
            px: 0,
            py: 0,
            hp: 20,
            maxhp: 20,
            atk: 3,
            depth: 1,
            kills: 0,
            monsters: Vec::new(),
            items: Vec::new(),
            msgs: Vec::new(),
            seed,
            combat_rng: channel(seed, &["combat"]),
            ai_rng: channel(seed, &["ai"]),
            dead: false,
            won: false,
        };
        g.gen_level();
        g.log(String::from("Welcome. Find the Amulet on depth 5!"));
        g
    }

    fn log(&mut self, s: String) {
        self.msgs.push(s);
        if self.msgs.len() > 40 {
            self.msgs.remove(0);
        }
    }

    /* ── WORLDGEN: pure f(seed, depth) via the "worldgen"/"spawns" channels.
       Output is frozen by the golden fixtures in tests/golden/: any diff to
       a golden is a seed-breaking MAJOR change requiring explicit human
       sign-off — never a drive-by. ──────────────────────────────────────── */
    fn gen_level(&mut self) {
        self.map = vec![Tile::Wall; COLS * MAP_H];
        self.seen = vec![false; COLS * MAP_H];
        self.vis = vec![false; COLS * MAP_H];
        self.monsters.clear();
        self.items.clear();

        let depth_tag = self.depth.to_string();
        let mut wr = channel(self.seed, &["worldgen", &depth_tag]);
        let mut sr = channel(self.seed, &["spawns", &depth_tag]);

        // rooms
        let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new(); // x,y,w,h
        for _ in 0..80 {
            if rooms.len() >= 10 {
                break;
            }
            let w = wr.range(5, 12);
            let h = wr.range(4, 8);
            let x = wr.range(1, COLS as i32 - w - 1);
            let y = wr.range(1, MAP_H as i32 - h - 1);
            let clash = rooms.iter().any(|&(rx, ry, rw, rh)| {
                x < rx + rw + 1 && rx < x + w + 1 && y < ry + rh + 1 && ry < y + h + 1
            });
            if clash {
                continue;
            }
            for cy in y..y + h {
                for cx in x..x + w {
                    self.map[idx(cx, cy)] = Tile::Floor;
                }
            }
            rooms.push((x, y, w, h));
        }
        // corridors between consecutive room centers (L-shaped)
        let centers: Vec<(i32, i32)> =
            rooms.iter().map(|&(x, y, w, h)| (x + w / 2, y + h / 2)).collect();
        for i in 1..centers.len() {
            let (ax, ay) = centers[i - 1];
            let (bx, by) = centers[i];
            let (mut cx, mut cy) = (ax, ay);
            let horiz_first = wr.chance(1, 2);
            while (cx, cy) != (bx, by) {
                if (horiz_first && cx != bx) || cy == by {
                    cx += (bx - cx).signum();
                } else {
                    cy += (by - cy).signum();
                }
                self.map[idx(cx, cy)] = Tile::Floor;
            }
        }

        let (sx, sy) = centers[0];
        self.px = sx;
        self.py = sy;
        let last = *centers.last().unwrap();
        if self.depth < MAX_DEPTH {
            self.map[idx(last.0, last.1)] = Tile::Stairs;
        } else {
            self.items.push(Item { x: last.0, y: last.1, kind: IKind::Amulet });
        }

        // monsters: scale with depth, spawn on floor away from player
        let count = 3 + self.depth as i32 * 2;
        for _ in 0..count {
            if let Some((mx, my)) = self.rand_floor(&mut sr, 8) {
                let roll = sr.range(0, 10) + self.depth as i32 * 2;
                let kind = if roll < 8 {
                    MKind::Rat
                } else if roll < 13 {
                    MKind::Goblin
                } else {
                    MKind::Ogre
                };
                let (hp, ..) = Monster::stats(kind);
                self.monsters.push(Monster { x: mx, y: my, kind, hp });
            }
        }
        // items
        for _ in 0..sr.range(2, 4) {
            if let Some((ix, iy)) = self.rand_floor(&mut sr, 4) {
                let kind = if sr.chance(3, 4) { IKind::Potion } else { IKind::Sword };
                self.items.push(Item { x: ix, y: iy, kind });
            }
        }
        self.compute_fov();
    }

    fn rand_floor(&mut self, rng: &mut Rng, min_player_dist: i32) -> Option<(i32, i32)> {
        for _ in 0..200 {
            let x = rng.range(1, COLS as i32 - 1);
            let y = rng.range(1, MAP_H as i32 - 1);
            if self.map[idx(x, y)] == Tile::Floor
                && (x - self.px).abs() + (y - self.py).abs() >= min_player_dist
                && !self.monsters.iter().any(|m| m.x == x && m.y == y)
                && !self.items.iter().any(|i| i.x == x && i.y == y)
            {
                return Some((x, y));
            }
        }
        None
    }

    // ---------- FOV: raycast to every tile within radius ----------
    fn compute_fov(&mut self) {
        self.vis.iter_mut().for_each(|v| *v = false);
        self.vis[idx(self.px, self.py)] = true;
        self.seen[idx(self.px, self.py)] = true;
        for dy in -FOV_R..=FOV_R {
            for dx in -FOV_R..=FOV_R {
                if dx * dx + dy * dy > FOV_R * FOV_R {
                    continue;
                }
                let (tx, ty) = (self.px + dx, self.py + dy);
                if !in_map(tx, ty) {
                    continue;
                }
                if self.los(self.px, self.py, tx, ty) {
                    self.vis[idx(tx, ty)] = true;
                    self.seen[idx(tx, ty)] = true;
                }
            }
        }
    }

    /// Bresenham line-of-sight; target tile itself may be a wall (so walls are visible).
    fn los(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = (x1 - x0).signum();
        let sy = (y1 - y0).signum();
        let mut err = dx + dy;
        loop {
            if (x, y) == (x1, y1) {
                return true;
            }
            if (x, y) != (x0, y0) && self.map[idx(x, y)] == Tile::Wall {
                return false;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    // ---------- Turn logic ----------
    fn try_move_player(&mut self, dx: i32, dy: i32) {
        if self.dead || self.won {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        if let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) {
            let dmg = self.atk + self.combat_rng.range(0, 3);
            let name = Monster::stats(self.monsters[mi].kind).4;
            self.monsters[mi].hp -= dmg;
            if self.monsters[mi].hp <= 0 {
                self.monsters.remove(mi);
                self.kills += 1;
                self.log(format!("You slay the {}! ({} dmg)", name, dmg));
            } else {
                self.log(format!("You hit the {} for {}.", name, dmg));
            }
        } else if self.map[idx(nx, ny)] != Tile::Wall {
            self.px = nx;
            self.py = ny;
            self.pickup();
            if self.map[idx(nx, ny)] == Tile::Stairs {
                self.depth += 1;
                self.log(format!("You descend to depth {}.", self.depth));
                self.gen_level();
                return; // fresh level: monsters don't get a free hit
            }
        } else {
            return; // bumped a wall: no turn passes
        }
        self.monsters_act();
        self.compute_fov();
    }

    fn wait_turn(&mut self) {
        if self.dead || self.won {
            return;
        }
        self.monsters_act();
        self.compute_fov();
    }

    fn pickup(&mut self) {
        if let Some(i) = self.items.iter().position(|i| i.x == self.px && i.y == self.py) {
            let kind = self.items[i].kind;
            self.items.remove(i);
            match kind {
                IKind::Potion => {
                    let heal = 8.min(self.maxhp - self.hp);
                    self.hp += heal;
                    self.log(format!("You quaff a potion. (+{} HP)", heal));
                }
                IKind::Sword => {
                    self.atk += 2;
                    self.log(String::from("A sharper sword! (+2 ATK)"));
                }
                IKind::Amulet => {
                    self.won = true;
                    self.log(String::from("The AMULET is yours! You win! [R] new run"));
                }
            }
        }
    }

    fn monsters_act(&mut self) {
        let (px, py) = (self.px, self.py);
        let mut attacks: Vec<(MKind, i32)> = Vec::new();
        for i in 0..self.monsters.len() {
            let (mx, my) = (self.monsters[i].x, self.monsters[i].y);
            let dist = (px - mx).abs().max((py - my).abs());
            let sees = dist <= FOV_R && self.los(mx, my, px, py);
            if dist == 1 && sees {
                let (_, atk, _, _, _) = Monster::stats(self.monsters[i].kind);
                let dmg = atk + self.combat_rng.range(0, 2);
                attacks.push((self.monsters[i].kind, dmg));
                continue;
            }
            let (dx, dy) = if sees {
                ((px - mx).signum(), (py - my).signum())
            } else if self.ai_rng.chance(1, 3) {
                (self.ai_rng.range(-1, 2), self.ai_rng.range(-1, 2))
            } else {
                (0, 0)
            };
            // try diagonal step, then each axis alone
            for (tx, ty) in [(mx + dx, my + dy), (mx + dx, my), (mx, my + dy)] {
                if (tx, ty) == (mx, my) {
                    continue;
                }
                if in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && (tx, ty) != (px, py)
                    && !self.monsters.iter().any(|m| m.x == tx && m.y == ty)
                {
                    self.monsters[i].x = tx;
                    self.monsters[i].y = ty;
                    break;
                }
            }
        }
        for (kind, dmg) in attacks {
            self.hp -= dmg;
            let name = Monster::stats(kind).4;
            if self.hp <= 0 {
                self.hp = 0;
                self.dead = true;
                self.log(format!("The {} kills you... [R] to restart", name));
                return;
            }
            self.log(format!("The {} hits you for {}.", name, dmg));
        }
    }
}

// ---------- Rendering ----------
fn draw_char(buf: &mut [u32], col: usize, row: usize, ch: u8, color: u32) {
    let glyph = BASIC_LEGACY[ch as usize & 0x7F];
    let ox = col * CW;
    let oy = row * CH + (CH - 8) / 2;
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..8 {
            if bits >> gx & 1 == 1 {
                buf[(oy + gy) * WIDTH + ox + gx] = color;
            }
        }
    }
}

fn draw_str(buf: &mut [u32], col: usize, row: usize, s: &str, color: u32) {
    for (i, ch) in s.bytes().enumerate() {
        if col + i >= COLS {
            break;
        }
        draw_char(buf, col + i, row, ch, color);
    }
}

fn dim(c: u32) -> u32 {
    (c >> 2) & 0x3F3F3F
}

fn render(g: &Game, buf: &mut [u32]) {
    buf.iter_mut().for_each(|p| *p = 0);
    // map
    for y in 0..MAP_H as i32 {
        for x in 0..COLS as i32 {
            let i = idx(x, y);
            if !g.seen[i] {
                continue;
            }
            let (ch, color) = match g.map[i] {
                Tile::Wall => (b'#', 0x9090A0),
                Tile::Floor => (b'.', 0x606068),
                Tile::Stairs => (b'>', 0xFFFF60),
            };
            let c = if g.vis[i] { color } else { dim(color) };
            draw_char(buf, x as usize, y as usize, ch, c);
        }
    }
    // items (visible only)
    for it in &g.items {
        if g.vis[idx(it.x, it.y)] {
            let (ch, c) = match it.kind {
                IKind::Potion => (b'!', 0xFF50A0),
                IKind::Sword => (b')', 0x70B0FF),
                IKind::Amulet => (b'&', 0xFFD700),
            };
            draw_char(buf, it.x as usize, it.y as usize, ch, c);
        }
    }
    // monsters (visible only)
    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            let (_, _, ch, c, _) = Monster::stats(m.kind);
            draw_char(buf, m.x as usize, m.y as usize, ch, c);
        }
    }
    // player
    draw_char(buf, g.px as usize, g.py as usize, b'@', 0xFFFFFF);

    // status
    let status = format!(
        "HP {:>2}/{}  ATK {}  Depth {}/{}  Kills {}",
        g.hp, g.maxhp, g.atk, g.depth, MAX_DEPTH, g.kills
    );
    let sc = if g.hp <= g.maxhp / 4 { 0xFF5050 } else { 0xE0E0E0 };
    draw_str(buf, 1, MAP_H, &status, sc);
    // log: last 4, older lines faded
    let n = g.msgs.len();
    let recent = &g.msgs[n.saturating_sub(4)..];
    let fade = [0x707070u32, 0x909090, 0xB0B0B0, 0xE0E0E0];
    for (r, m) in recent.iter().enumerate() {
        let shade = fade[fade.len() - recent.len() + r];
        draw_str(buf, 1, MAP_H + 1 + r, m, shade);
    }
}

// ---------- Headless dump for testing ----------
fn level_dump(g: &Game) -> String {
    let mut out = String::new();
    for y in 0..MAP_H as i32 {
        for x in 0..COLS as i32 {
            let mut ch = match g.map[idx(x, y)] {
                Tile::Wall => '#',
                Tile::Floor => '.',
                Tile::Stairs => '>',
            };
            for it in &g.items {
                if (it.x, it.y) == (x, y) {
                    ch = match it.kind {
                        IKind::Potion => '!',
                        IKind::Sword => ')',
                        IKind::Amulet => '&',
                    };
                }
            }
            for m in &g.monsters {
                if (m.x, m.y) == (x, y) {
                    ch = Monster::stats(m.kind).2 as char;
                }
            }
            if (g.px, g.py) == (x, y) {
                ch = '@';
            }
            out.push(ch);
        }
        out.push('\n');
    }
    out.push_str(&format!("monsters={} items={}\n", g.monsters.len(), g.items.len()));
    out
}

// ---------- Solver: winnability + walk-budget gate ----------
/// BFS distances (4-dir, walls block) from `from` over a level map.
fn bfs_dist(map: &[Tile], from: (i32, i32)) -> Vec<i32> {
    let mut dist = vec![-1i32; COLS * MAP_H];
    let mut q = std::collections::VecDeque::new();
    dist[idx(from.0, from.1)] = 0;
    q.push_back(from);
    while let Some((cx, cy)) = q.pop_front() {
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if in_map(nx, ny) && map[idx(nx, ny)] != Tile::Wall && dist[idx(nx, ny)] < 0 {
                dist[idx(nx, ny)] = dist[idx(cx, cy)] + 1;
                q.push_back((nx, ny));
            }
        }
    }
    dist
}

/// Round-trip walk budget for one seed: for each depth, BFS from the entry
/// to the exit (down-stairs, or the amulet on the last depth). budget =
/// 3 × total shortest path — walk in ×1, carry the amulet out ×2 (it is
/// heavy). Returns None if any exit is unreachable (unwinnable seed).
fn solve_seed(seed: u64) -> Option<i32> {
    let mut g = Game::new(seed);
    let mut total = 0;
    for d in 1..=MAX_DEPTH {
        g.depth = d;
        g.gen_level();
        let target = if d < MAX_DEPTH {
            let s = (0..COLS as i32 * MAP_H as i32).find_map(|i| {
                let (x, y) = (i % COLS as i32, i / COLS as i32);
                if g.map[idx(x, y)] == Tile::Stairs { Some((x, y)) } else { None }
            });
            s?
        } else {
            let a = g.items.iter().find(|it| it.kind == IKind::Amulet)?;
            (a.x, a.y)
        };
        let dd = bfs_dist(&g.map, (g.px, g.py))[idx(target.0, target.1)];
        if dd < 0 {
            return None;
        }
        total += dd;
    }
    Some(3 * total)
}

/// Pull `"key": [lo, hi]` out of the band file without a JSON crate.
fn band_range(json: &str, key: &str) -> Option<(i32, i32)> {
    let k = format!("\"{}\"", key);
    let rest = &json[json.find(&k)? + k.len()..];
    let body = &rest[rest.find('[')? + 1..];
    let body = &body[..body.find(']')?];
    let mut it = body.split(',');
    Some((it.next()?.trim().parse().ok()?, it.next()?.trim().parse().ok()?))
}

/// `--solve N`: winnability + difficulty gate over seeds 0..N. Prints JSON
/// stats; exits nonzero on any unwinnable seed or drift outside the band
/// committed in tests/solver-band.json.
fn solve_main(n: u64) {
    let mut budgets: Vec<i32> = Vec::new();
    let mut losers: Vec<u64> = Vec::new();
    let (mut worst_seed, mut worst) = (0u64, -1i32);
    for seed in 0..n {
        match solve_seed(seed) {
            Some(b) => {
                if b > worst {
                    worst = b;
                    worst_seed = seed;
                }
                budgets.push(b);
            }
            None => losers.push(seed),
        }
    }
    budgets.sort_unstable();
    let pct = |p: usize| budgets[p * (budgets.len() - 1) / 100];
    let stats = [
        ("min", budgets[0]),
        ("p50", pct(50)),
        ("p90", pct(90)),
        ("p99", pct(99)),
        ("max", worst),
    ];
    println!("{{");
    println!("  \"seeds\": {},", n);
    for (k, v) in stats {
        println!("  \"{}\": {},", k, v);
    }
    println!("  \"worstSeed\": {},", worst_seed);
    println!("  \"unwinnable\": {}", losers.len());
    println!("}}");
    if !losers.is_empty() {
        eprintln!("UNWINNABLE: {}/{} seeds, e.g. {:?}", losers.len(), n, &losers[..losers.len().min(5)]);
        std::process::exit(1);
    }
    match std::fs::read_to_string("tests/solver-band.json") {
        Ok(band) => {
            for (k, v) in stats {
                if let Some((lo, hi)) = band_range(&band, k) {
                    if v < lo || v > hi {
                        eprintln!("difficulty drift: {}={} outside [{},{}]", k, v, lo, hi);
                        std::process::exit(1);
                    }
                }
            }
            println!("solver: {} seeds winnable, difficulty band OK", n);
        }
        Err(_) => {
            eprintln!("warning: tests/solver-band.json not found; drift check skipped");
        }
    }
}

/// Full-run dump: every depth of the seed's dungeon, generated directly.
fn dump(seed: u64) -> String {
    let mut g = Game::new(seed);
    let mut out = format!("seed={}\n", seed);
    for d in 1..=MAX_DEPTH {
        g.depth = d;
        g.gen_level();
        out.push_str(&format!("-- depth {} --\n", d));
        out.push_str(&level_dump(&g));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag_val = |name: &str| -> Option<u64> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
    };
    let seed = flag_val("--seed").unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF)
    });

    if args.iter().any(|a| a == "--solve") {
        solve_main(flag_val("--solve").unwrap_or(10000));
        return;
    }
    if args.iter().any(|a| a == "--dump") {
        print!("{}", dump(seed));
        return;
    }

    let mut game = Game::new(seed);
    let mut window = Window::new(
        "rl144",
        WIDTH,
        HEIGHT,
        WindowOptions { resize: false, ..WindowOptions::default() },
    )
    .expect("window");
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    let mut buf = vec![0u32; WIDTH * HEIGHT];
    let moves: [(Key, (i32, i32)); 12] = [
        (Key::Up, (0, -1)),
        (Key::Down, (0, 1)),
        (Key::Left, (-1, 0)),
        (Key::Right, (1, 0)),
        (Key::W, (0, -1)),
        (Key::S, (0, 1)),
        (Key::A, (-1, 0)),
        (Key::D, (1, 0)),
        (Key::K, (0, -1)),
        (Key::J, (0, 1)),
        (Key::H, (-1, 0)),
        (Key::L, (1, 0)),
    ];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for (key, (dx, dy)) in moves {
            if window.is_key_pressed(key, KeyRepeat::Yes) {
                game.try_move_player(dx, dy);
            }
        }
        if window.is_key_pressed(Key::Period, KeyRepeat::Yes) {
            game.wait_turn();
        }
        if (game.dead || game.won) && window.is_key_pressed(Key::R, KeyRepeat::No) {
            let s = h64(game.seed, &["restart"]);
            game = Game::new(s);
        }
        render(&game, &mut buf);
        window.update_with_buffer(&buf, WIDTH, HEIGHT).expect("update");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task-1 acceptance: combat/AI channel draws must not perturb worldgen.
    #[test]
    fn worldgen_isolated_from_combat() {
        let seed = 0xC0FFEE;
        let mut a = Game::new(seed); // generates depth 1
        for _ in 0..1000 {
            a.combat_rng.next();
            a.ai_rng.next();
        }
        a.depth = 2;
        a.gen_level();
        let dirty = level_dump(&a);

        let mut b = Game::new(seed);
        b.depth = 2;
        b.gen_level();
        assert_eq!(dirty, level_dump(&b));
    }

    /// Same seed, same full dungeon — twice.
    #[test]
    fn dump_deterministic() {
        assert_eq!(dump(42), dump(42));
    }

    /// Worldgen output is FROZEN by these fixtures. A diff here means a
    /// seed-breaking MAJOR change: get explicit human sign-off, then
    /// regenerate with `--dump --seed N > tests/golden/seed_N.txt`.
    #[test]
    fn golden_dumps() {
        let goldens: [(u64, &str); 5] = [
            (1, include_str!("../tests/golden/seed_1.txt")),
            (2, include_str!("../tests/golden/seed_2.txt")),
            (3, include_str!("../tests/golden/seed_3.txt")),
            (42, include_str!("../tests/golden/seed_42.txt")),
            (1337, include_str!("../tests/golden/seed_1337.txt")),
        ];
        for (seed, want) in goldens {
            assert_eq!(dump(seed), want, "golden drift for seed {}", seed);
        }
    }

    /// Every depth of every small seed must be winnable (exit reachable).
    #[test]
    fn solver_smoke() {
        for seed in 0..50 {
            assert!(solve_seed(seed).is_some(), "seed {} unwinnable", seed);
        }
    }
}
