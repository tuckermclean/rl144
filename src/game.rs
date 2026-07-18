// game.rs — the engine core: grid/game constants, world state (Tile, Monster,
// Item, LevelState, Game) and all Game behavior (worldgen, FOV, turn logic,
// combat, save/restore of visited depths), plus the grid helpers idx/in_map
// and the solver's bfs_dist. apply_input is the client input-vocabulary
// boundary between this engine and any frontend (window loop, replay, sim).

use crate::content::{KINDS, THEMES, TONE_LINES, Theme, VAULTS, theme_pick};
use crate::rng::{Rng, channel};

pub(crate) const COLS: usize = 80;
pub(crate) const ROWS: usize = 30;
pub(crate) const MAP_H: usize = 25; // rows [0,25) map; row 25 status; rows [26,30) log
pub(crate) const MAX_DEPTH: u32 = 5;
const MONSTER_SIGHT: i32 = 8;

/* Solver-derived: worst-case round-trip walk budget over the 10K CI seed
   set is 1503 (--solve 10000, worstSeed 6592; budget = descend ×1 + climb
   out ×2 per step, see solve_seed). Start light 2000 leaves ~33% margin for
   combat, detours and loot runs. Changing this re-tunes every run: rerun
   --solve and re-commit tests/solver-band.json alongside it. */
const START_LIGHT: i32 = 2000;

/// Player FOV radius shrinks as the torch burns down. Percent of START_LIGHT.
pub(crate) fn fov_radius(light: i32) -> i32 {
    let pct = light * 100 / START_LIGHT;
    match pct {
        _ if pct > 50 => 8,
        _ if pct > 30 => 6,
        _ if pct > 18 => 5,
        _ if pct > 10 => 4,
        _ if pct > 4 => 3,
        _ => 2,
    }
}

// ---------- Map ----------
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tile {
    Wall,
    Floor,
    Stairs,   // '>' down
    UpStairs, // '<' up; on depth 1 it is the way out (win tile, amulet in hand)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MKind {
    Rat,
    Goblin,
    Ogre,
}

#[derive(Clone)]
pub(crate) struct Monster {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) kind: MKind,
    pub(crate) hp: i32,
}

impl Monster {
    pub(crate) fn stats(kind: MKind) -> (i32, i32, u8, u32, &'static str) {
        // (hp, atk, glyph, color, name)
        match kind {
            MKind::Rat => (3, 1, b'r', 0xB0703A, "rat"),
            MKind::Goblin => (6, 2, b'g', 0x40C040, "goblin"),
            MKind::Ogre => (13, 4, b'O', 0xD05050, "ogre"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum IKind {
    Potion,
    Sword,
    Amulet,
    LoreA, // shallow-tier inscription: theme lore template 0
    LoreB, // mid-tier: template 1
    LoreC, // deep-tier: template 2
}

impl IKind {
    pub(crate) fn lore_tier(self) -> Option<usize> {
        match self {
            IKind::LoreA => Some(0),
            IKind::LoreB => Some(1),
            IKind::LoreC => Some(2),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Item {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) kind: IKind,
}

/// Snapshot of a visited depth, so the climb back out with the amulet is
/// through the world you left: same layout, taken items stay taken, dead
/// monsters stay dead, live ones where they last stood.
pub(crate) struct LevelState {
    pub(crate) map: Vec<Tile>,
    pub(crate) seen: Vec<bool>,
    pub(crate) monsters: Vec<Monster>,
    pub(crate) items: Vec<Item>,
    pub(crate) rooms: Vec<(i32, i32, i32, i32)>,
    pub(crate) room_meta: Vec<(u8, u8)>, // (kind, tone) indices
    pub(crate) room_visited: Vec<bool>,
}

pub(crate) struct Game {
    pub(crate) map: Vec<Tile>,
    pub(crate) seen: Vec<bool>,
    pub(crate) vis: Vec<bool>,
    pub(crate) px: i32,
    pub(crate) py: i32,
    pub(crate) hp: i32,
    pub(crate) maxhp: i32,
    pub(crate) atk: i32,
    pub(crate) depth: u32,
    pub(crate) kills: u32,
    pub(crate) light: i32,
    pub(crate) has_amulet: bool,
    pub(crate) monsters: Vec<Monster>,
    pub(crate) items: Vec<Item>,
    pub(crate) rooms: Vec<(i32, i32, i32, i32)>,
    pub(crate) room_meta: Vec<(u8, u8)>,
    pub(crate) room_visited: Vec<bool>,
    pub(crate) saved: Vec<Option<LevelState>>,
    pub(crate) msgs: Vec<String>,
    pub(crate) seed: u64,
    pub(crate) combat_rng: Rng,
    pub(crate) ai_rng: Rng,
    pub(crate) flavor_rng: Rng,
    pub(crate) dead: bool,
    pub(crate) won: bool,
}

pub(crate) fn idx(x: i32, y: i32) -> usize {
    y as usize * COLS + x as usize
}
pub(crate) fn in_map(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < COLS as i32 && y < MAP_H as i32
}

impl Game {
    pub(crate) fn new(seed: u64) -> Self {
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
            light: START_LIGHT,
            has_amulet: false,
            monsters: Vec::new(),
            items: Vec::new(),
            rooms: Vec::new(),
            room_meta: Vec::new(),
            room_visited: Vec::new(),
            saved: (0..MAX_DEPTH).map(|_| None).collect(),
            msgs: Vec::new(),
            seed,
            combat_rng: channel(seed, &["combat"]),
            ai_rng: channel(seed, &["ai"]),
            flavor_rng: channel(seed, &["flavor"]),
            dead: false,
            won: false,
        };
        g.gen_level();
        g.log(String::from("Fetch the Amulet from depth 5 and climb back before dark!"));
        g
    }

    pub(crate) fn theme(&self) -> &'static Theme {
        &THEMES[theme_pick(self.seed, self.depth).0]
    }

    /// The filled lore line for a tier of the current depth's theme.
    fn lore_line(&self, tier: usize) -> String {
        let (ti, slots) = theme_pick(self.seed, self.depth);
        let t = &THEMES[ti];
        t.lore[tier].replace("{A}", t.slots[slots[tier]])
    }

    fn mob_name(&self, k: MKind) -> &'static str {
        self.theme().mobs[k as usize]
    }

    fn adj(&mut self) -> &'static str {
        let t = self.theme();
        t.adjs[self.flavor_rng.range(0, t.adjs.len() as i32) as usize]
    }

    pub(crate) fn log(&mut self, s: String) {
        self.msgs.push(s);
        if self.msgs.len() > 40 {
            self.msgs.remove(0);
        }
    }

    /* ── WORLDGEN: pure f(seed, depth) via the "worldgen"/"spawns" channels.
       Output is frozen by the golden fixtures in tests/golden/: any diff to
       a golden is a seed-breaking MAJOR change requiring explicit human
       sign-off — never a drive-by. ──────────────────────────────────────── */
    pub(crate) fn gen_level(&mut self) {
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

        // occasionally stamp one hand-authored vault as an extra room; the
        // corridor pass below connects its center like any other room
        let mut vault_room: Option<usize> = None;
        let mut vr = channel(self.seed, &["vault", &depth_tag]);
        if vr.chance(2, 5) {
            let rows: Vec<&str> =
                VAULTS[vr.range(0, VAULTS.len() as i32) as usize].lines().collect();
            let (vw, vh) = (rows[0].len() as i32, rows.len() as i32);
            for _ in 0..40 {
                let x = vr.range(1, COLS as i32 - vw - 1);
                let y = vr.range(1, MAP_H as i32 - vh - 1);
                let clash = rooms.iter().any(|&(rx, ry, rw, rh)| {
                    x < rx + rw + 1 && rx < x + vw + 1 && y < ry + rh + 1 && ry < y + vh + 1
                });
                if clash {
                    continue;
                }
                for (j, row) in rows.iter().enumerate() {
                    for (i, c) in row.bytes().enumerate() {
                        let (tx, ty) = (x + i as i32, y + j as i32);
                        if c == b'#' {
                            continue; // already wall
                        }
                        self.map[idx(tx, ty)] = Tile::Floor;
                        match c {
                            b'!' => self.items.push(Item { x: tx, y: ty, kind: IKind::Potion }),
                            b')' => self.items.push(Item { x: tx, y: ty, kind: IKind::Sword }),
                            b'r' | b'g' | b'O' => {
                                let kind = match c {
                                    b'r' => MKind::Rat,
                                    b'g' => MKind::Goblin,
                                    _ => MKind::Ogre,
                                };
                                let (hp, ..) = Monster::stats(kind);
                                self.monsters.push(Monster { x: tx, y: ty, kind, hp });
                            }
                            _ => {}
                        }
                    }
                }
                vault_room = Some(rooms.len());
                rooms.push((x, y, vw, vh));
                break;
            }
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
        // BFS depth from the entrance is the level's act structure: the exit
        // (down-stairs, or the amulet on the last depth) goes in the DEEPEST
        // reachable room, not the last-generated one.
        let dist = bfs_dist(&self.map, (sx, sy));
        let deepest = centers
            .iter()
            .filter(|&&c| c != (sx, sy))
            .max_by_key(|&&(cx, cy)| dist[idx(cx, cy)])
            .copied()
            .unwrap_or((sx + 1, sy));
        self.map[idx(sx, sy)] = Tile::UpStairs;
        if self.depth < MAX_DEPTH {
            self.map[idx(deepest.0, deepest.1)] = Tile::Stairs;
        } else {
            self.items.push(Item { x: deepest.0, y: deepest.1, kind: IKind::Amulet });
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
        // story buried by depth: three inscriptions at shallow / mid / deep
        // rooms (by BFS distance from the entrance), read on walk-over.
        // Placement is a pure function of `dist` — zero extra RNG draws.
        let mut by_depth = centers.clone();
        by_depth.sort_by_key(|&(cx, cy)| dist[idx(cx, cy)]);
        let n = by_depth.len();
        let picks = [by_depth[1.min(n - 1)], by_depth[n / 2], by_depth[n.saturating_sub(2)]];
        for (tier, &(cx, cy)) in picks.iter().enumerate() {
            if (cx, cy) == deepest
                || (cx, cy) == (sx, sy)
                || self.map[idx(cx, cy)] != Tile::Floor
                || self.items.iter().any(|it| (it.x, it.y) == (cx, cy))
                || self.monsters.iter().any(|m| (m.x, m.y) == (cx, cy))
            {
                continue;
            }
            let kind = [IKind::LoreA, IKind::LoreB, IKind::LoreC][tier];
            self.items.push(Item { x: cx, y: cy, kind });
        }

        // room identity: kind + tone per room from the "tone" channel (its
        // own stream — adds zero draws to worldgen/spawns, so layouts and
        // goldens are untouched). Spawn room counts as already entered.
        let mut tn = channel(self.seed, &["tone", &depth_tag]);
        self.room_meta = rooms
            .iter()
            .map(|_| (tn.range(0, 6) as u8, tn.range(0, 4) as u8))
            .collect();
        if let Some(vi) = vault_room {
            self.room_meta[vi].0 = 6; // forced "vault"
        }
        self.room_visited = vec![false; rooms.len()];
        self.room_visited[0] = true;
        self.rooms = rooms;

        self.compute_fov();

        // first arrival: name the place; its history is buried in the rooms
        let t = self.theme();
        self.log(format!("You enter {}.", t.label));
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
        let r = fov_radius(self.light);
        self.vis.iter_mut().for_each(|v| *v = false);
        self.vis[idx(self.px, self.py)] = true;
        self.seen[idx(self.px, self.py)] = true;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
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
    /// Burn the torch for one player turn: 1 light, 2 while carrying the
    /// amulet (it is heavy). Light 0 is death in the dark — checked before
    /// any win condition, golem-style. Returns false if the player died.
    fn spend_turn(&mut self) -> bool {
        let before = fov_radius(self.light);
        self.light -= if self.has_amulet { 2 } else { 1 };
        if self.light <= 0 {
            self.light = 0;
            self.dead = true;
            self.log(String::from("Your torch dies. The darkness takes you. [R] to restart"));
            self.compute_fov();
            return false;
        }
        let after = fov_radius(self.light);
        if after < before {
            let warn = match after {
                6 => "Your torch burns low. The shadows edge closer.",
                5 => "The flame gutters; you can see less and less.",
                4 => "Darkness presses in around your failing light.",
                3 => "Your torch is nearly spent. Hurry.",
                _ => "The last embers. The dark is almost total.",
            };
            self.log(String::from(warn));
        }
        true
    }

    /// Snapshot the current depth so it persists exactly as left.
    fn stash_level(&mut self) {
        let d = self.depth as usize - 1;
        self.saved[d] = Some(LevelState {
            map: std::mem::take(&mut self.map),
            seen: std::mem::take(&mut self.seen),
            monsters: std::mem::take(&mut self.monsters),
            items: std::mem::take(&mut self.items),
            rooms: std::mem::take(&mut self.rooms),
            room_meta: std::mem::take(&mut self.room_meta),
            room_visited: std::mem::take(&mut self.room_visited),
        });
    }

    /// Restore a previously visited depth and place the player at `arrive`.
    /// A monster that wandered onto the arrival tile is shoved aside.
    fn restore_level(&mut self, ls: LevelState, arrive: Tile) {
        self.map = ls.map;
        self.seen = ls.seen;
        self.monsters = ls.monsters;
        self.items = ls.items;
        self.rooms = ls.rooms;
        self.room_meta = ls.room_meta;
        self.room_visited = ls.room_visited;
        self.vis = vec![false; COLS * MAP_H];
        let pos = (0..COLS as i32 * MAP_H as i32)
            .map(|i| (i % COLS as i32, i / COLS as i32))
            .find(|&(x, y)| self.map[idx(x, y)] == arrive)
            .unwrap_or((self.px, self.py));
        self.px = pos.0;
        self.py = pos.1;
        if let Some(mi) = self.monsters.iter().position(|m| (m.x, m.y) == pos) {
            let (mx, my) = (self.monsters[mi].x, self.monsters[mi].y);
            let spot = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().find_map(|&(dx, dy)| {
                let (tx, ty) = (mx + dx, my + dy);
                let free = in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && !self.monsters.iter().any(|m| (m.x, m.y) == (tx, ty));
                if free { Some((tx, ty)) } else { None }
            });
            match spot {
                Some((tx, ty)) => {
                    self.monsters[mi].x = tx;
                    self.monsters[mi].y = ty;
                }
                None => {
                    self.monsters.remove(mi);
                }
            }
        }
        self.compute_fov();
    }

    pub(crate) fn descend(&mut self) {
        self.stash_level();
        self.depth += 1;
        let d = self.depth as usize - 1;
        match self.saved[d].take() {
            Some(ls) => self.restore_level(ls, Tile::UpStairs),
            None => self.gen_level(),
        }
        self.log(format!("You descend to depth {}.", self.depth));
    }

    pub(crate) fn ascend(&mut self) {
        self.stash_level();
        self.depth -= 1;
        let d = self.depth as usize - 1;
        match self.saved[d].take() {
            Some(ls) => self.restore_level(ls, Tile::Stairs),
            None => self.gen_level(), // unreachable in play; belt and braces
        }
        self.log(format!("You climb back to depth {}.", self.depth));
    }

    /// First step into a room surfaces its tone line, once per level visit.
    fn note_room_entry(&mut self) {
        let (px, py) = (self.px, self.py);
        let ri = self.rooms.iter().position(|&(rx, ry, rw, rh)| {
            px >= rx && px < rx + rw && py >= ry && py < ry + rh
        });
        if let Some(ri) = ri {
            if !self.room_visited[ri] {
                self.room_visited[ri] = true;
                let (k, t) = self.room_meta[ri];
                let line = TONE_LINES[t as usize][self.flavor_rng.range(0, 2) as usize]
                    .replace("{K}", KINDS[k as usize]);
                self.log(line);
            }
        }
    }

    pub(crate) fn try_move_player(&mut self, dx: i32, dy: i32) {
        if self.dead || self.won {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        if let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) {
            let dmg = self.atk + self.combat_rng.range(0, 3);
            let name = self.mob_name(self.monsters[mi].kind);
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
            if !self.spend_turn() {
                return; // died in the dark: lose beats anything this tile offered
            }
            self.note_room_entry();
            self.pickup();
            match self.map[idx(nx, ny)] {
                Tile::Stairs => {
                    self.descend();
                    return; // fresh level: monsters don't get a free hit
                }
                Tile::UpStairs => {
                    if self.depth > 1 {
                        self.ascend();
                        return; // same courtesy on arrival upstairs
                    }
                    if self.has_amulet {
                        self.won = true;
                        self.log(String::from("You climb into daylight. You made it! [R] new run"));
                        return;
                    }
                    self.log(String::from("The way out. You won't leave without the Amulet."));
                }
                _ => {}
            }
            self.monsters_act();
            self.compute_fov();
            return;
        } else {
            return; // bumped a wall: no turn passes
        }
        // attack path: the swing costs a turn too
        if !self.spend_turn() {
            return;
        }
        self.monsters_act();
        self.compute_fov();
    }

    pub(crate) fn wait_turn(&mut self) {
        if self.dead || self.won {
            return;
        }
        if !self.spend_turn() {
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
                    let a = self.adj();
                    self.log(format!("You quaff a {} draught. (+{} HP)", a, heal));
                }
                IKind::Sword => {
                    self.atk += 2;
                    let a = self.adj();
                    self.log(format!("A {} blade, still sharp! (+2 ATK)", a));
                }
                IKind::Amulet => {
                    self.has_amulet = true;
                    let name = self.theme().amulet;
                    self.log(format!("You take {}. It is heavy. Climb, before dark!", name));
                }
                IKind::LoreA | IKind::LoreB | IKind::LoreC => {
                    let line = self.lore_line(kind.lore_tier().unwrap());
                    self.log(String::from("A carved inscription:"));
                    self.log(line);
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
            let sees = dist <= MONSTER_SIGHT && self.los(mx, my, px, py);
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
            let name = self.mob_name(kind);
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

impl Game {
    pub(crate) fn apply_input(&mut self, b: u8) {
        match b {
            0 => self.try_move_player(0, -1),
            1 => self.try_move_player(0, 1),
            2 => self.try_move_player(-1, 0),
            3 => self.try_move_player(1, 0),
            4 => self.wait_turn(),
            _ => {}
        }
    }
}

/// BFS distances (4-dir, walls block) from `from` over a level map.
pub(crate) fn bfs_dist(map: &[Tile], from: (i32, i32)) -> Vec<i32> {
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
