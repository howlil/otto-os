use std::fmt::Write;

pub const CONTROL_GRID_COLUMNS: u8 = 4;
pub const CONTROL_GRID_MAX_ROWS: u8 = 12;
pub const CONTROL_GRID_MAX_TILES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlSize {
    pub cols: u8,
    pub rows: u8,
}

impl ControlSize {
    #[must_use]
    pub const fn new(cols: u8, rows: u8) -> Self {
        Self { cols, rows }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match (self.cols, self.rows) {
            (1, 1) => "1×1",
            (2, 1) => "2×1",
            (1, 2) => "1×2",
            (2, 2) => "2×2",
            (4, 1) => "4×1",
            (4, 2) => "4×2",
            _ => "Custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCategory {
    Connectivity,
    DisplaySound,
    MediaFocus,
    SystemOtto,
}

impl ControlCategory {
    pub const ALL: &'static [Self] = &[
        Self::Connectivity,
        Self::DisplaySound,
        Self::MediaFocus,
        Self::SystemOtto,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connectivity => "Connectivity",
            Self::DisplaySound => "Display & Sound",
            Self::MediaFocus => "Media & Focus",
            Self::SystemOtto => "System & Otto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Connectivity,
    Wifi,
    Bluetooth,
    NowPlaying,
    Brightness,
    Volume,
    Focus,
    Battery,
    Mute,
    NightLight,
    Appearance,
    Screenshot,
    LockScreen,
    TaskView,
    Finder,
    Spotlight,
    Settings,
}

impl ControlKind {
    pub const ALL: &'static [Self] = &[
        Self::Connectivity,
        Self::Wifi,
        Self::Bluetooth,
        Self::Brightness,
        Self::Volume,
        Self::Mute,
        Self::NightLight,
        Self::NowPlaying,
        Self::Focus,
        Self::Battery,
        Self::Appearance,
        Self::Screenshot,
        Self::LockScreen,
        Self::TaskView,
        Self::Finder,
        Self::Spotlight,
        Self::Settings,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Connectivity => "connectivity",
            Self::Wifi => "wifi",
            Self::Bluetooth => "bluetooth",
            Self::NowPlaying => "now_playing",
            Self::Brightness => "brightness",
            Self::Volume => "volume",
            Self::Focus => "focus",
            Self::Battery => "battery",
            Self::Mute => "mute",
            Self::NightLight => "night_light",
            Self::Appearance => "appearance",
            Self::Screenshot => "screenshot",
            Self::LockScreen => "lock_screen",
            Self::TaskView => "task_view",
            Self::Finder => "finder",
            Self::Spotlight => "spotlight",
            Self::Settings => "settings",
        }
    }

    #[must_use]
    pub fn from_key(raw: &str) -> Option<Self> {
        let normalized = raw.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.key().eq_ignore_ascii_case(normalized))
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connectivity => "Connectivity",
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::NowPlaying => "Now Playing",
            Self::Brightness => "Display",
            Self::Volume => "Sound",
            Self::Focus => "Focus",
            Self::Battery => "Battery",
            Self::Mute => "Mute Audio",
            Self::NightLight => "Night Light",
            Self::Appearance => "Dark Mode",
            Self::Screenshot => "Screenshot",
            Self::LockScreen => "Lock Screen",
            Self::TaskView => "Task View",
            Self::Finder => "Otto Finder",
            Self::Spotlight => "Spotlight",
            Self::Settings => "Otto Settings",
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::Connectivity => "Wi-Fi & Bluetooth pack",
            Self::Wifi => "Toggle or pick a network",
            Self::Bluetooth => "Toggle or pair devices",
            Self::NowPlaying => "Playback & album artwork",
            Self::Brightness => "Display brightness slider",
            Self::Volume => "System volume slider",
            Self::Focus => "Focus Pomodoro & shield",
            Self::Battery => "Charge & power mode",
            Self::Mute => "Mute or unmute speakers",
            Self::NightLight => "Windows Night Light settings",
            Self::Appearance => "Switch Dark or Light theme",
            Self::Screenshot => "Capture screen snip",
            Self::LockScreen => "Lock Windows session",
            Self::TaskView => "Open Windows Task View",
            Self::Finder => "Open Otto Finder",
            Self::Spotlight => "Open Spotlight search",
            Self::Settings => "Open Otto Settings",
        }
    }

    #[must_use]
    pub const fn category(self) -> ControlCategory {
        match self {
            Self::Connectivity | Self::Wifi | Self::Bluetooth => ControlCategory::Connectivity,
            Self::Brightness | Self::Volume | Self::Mute | Self::NightLight => {
                ControlCategory::DisplaySound
            }
            Self::NowPlaying | Self::Focus => ControlCategory::MediaFocus,
            Self::Battery
            | Self::Appearance
            | Self::Screenshot
            | Self::LockScreen
            | Self::TaskView
            | Self::Finder
            | Self::Spotlight
            | Self::Settings => ControlCategory::SystemOtto,
        }
    }

    #[must_use]
    pub const fn default_size(self) -> ControlSize {
        self.supported_sizes()[0]
    }

    #[must_use]
    pub const fn supported_sizes(self) -> &'static [ControlSize] {
        const PACK: &[ControlSize] = &[
            ControlSize::new(2, 2),
            ControlSize::new(2, 1),
            ControlSize::new(1, 1),
        ];
        const TILE_STD: &[ControlSize] = &[
            ControlSize::new(1, 1),
            ControlSize::new(2, 1),
            ControlSize::new(2, 2),
        ];
        const SLIDER: &[ControlSize] = &[
            ControlSize::new(1, 2),
            ControlSize::new(2, 1),
            ControlSize::new(4, 1),
        ];
        const MEDIA: &[ControlSize] = &[
            ControlSize::new(2, 2),
            ControlSize::new(4, 1),
            ControlSize::new(4, 2),
        ];
        const ACTION: &[ControlSize] = &[ControlSize::new(1, 1), ControlSize::new(2, 1)];

        match self {
            Self::Connectivity => PACK,
            Self::Wifi | Self::Bluetooth | Self::Focus | Self::Battery => TILE_STD,
            Self::Brightness | Self::Volume => SLIDER,
            Self::NowPlaying => MEDIA,
            Self::Mute
            | Self::NightLight
            | Self::Appearance
            | Self::Screenshot
            | Self::LockScreen
            | Self::TaskView
            | Self::Finder
            | Self::Spotlight
            | Self::Settings => ACTION,
        }
    }

    #[must_use]
    pub fn snap_size(self, desired_cols: u8, desired_rows: u8) -> ControlSize {
        let supported = self.supported_sizes();
        let mut best = supported[0];
        let mut best_score = u16::MAX;
        for &candidate in supported {
            let dc = u16::from(candidate.cols.abs_diff(desired_cols));
            let dr = u16::from(candidate.rows.abs_diff(desired_rows));
            let score = dc * 10 + dr * 10;
            if score < best_score {
                best_score = score;
                best = candidate;
            }
        }
        best
    }

    #[must_use]
    pub fn next_size(self, current: ControlSize) -> ControlSize {
        let supported = self.supported_sizes();
        let pos = supported
            .iter()
            .position(|size| *size == current)
            .unwrap_or(0);
        supported[(pos + 1) % supported.len()]
    }

    #[must_use]
    pub fn supported_sizes_summary(self) -> String {
        self.supported_sizes()
            .iter()
            .map(|size| size.label())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlTile {
    pub id: u32,
    pub kind: ControlKind,
    pub col: u8,
    pub row: u8,
    pub cols: u8,
    pub rows: u8,
}

impl ControlTile {
    #[must_use]
    pub const fn size(self) -> ControlSize {
        ControlSize::new(self.cols, self.rows)
    }

    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        let a_right = self.col + self.cols;
        let a_bottom = self.row + self.rows;
        let b_right = other.col + other.cols;
        let b_bottom = other.row + other.rows;
        self.col < b_right && other.col < a_right && self.row < b_bottom && other.row < a_bottom
    }

    #[must_use]
    pub const fn covers(&self, col: u8, row: u8) -> bool {
        col >= self.col
            && col < self.col + self.cols
            && row >= self.row
            && row < self.row + self.rows
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCenterGrid {
    pub tiles: Vec<ControlTile>,
    next_id: u32,
}

impl Default for ControlCenterGrid {
    fn default() -> Self {
        Self::default_ipad_grid()
    }
}

impl ControlCenterGrid {
    #[must_use]
    pub fn default_ipad_grid() -> Self {
        let specs = [
            (ControlKind::Connectivity, 0, 0, 2, 2),
            (ControlKind::NowPlaying, 2, 0, 2, 2),
            (ControlKind::Focus, 0, 2, 1, 1),
            (ControlKind::Appearance, 0, 3, 1, 1),
            (ControlKind::Brightness, 1, 2, 1, 2),
            (ControlKind::Volume, 2, 2, 1, 2),
            (ControlKind::Battery, 3, 2, 1, 1),
            (ControlKind::LockScreen, 3, 3, 1, 1),
            (ControlKind::Screenshot, 0, 4, 2, 1),
            (ControlKind::Settings, 2, 4, 2, 1),
        ];
        let mut tiles = Vec::with_capacity(specs.len());
        let mut next_id = 1u32;
        for (kind, col, row, cols, rows) in specs {
            tiles.push(ControlTile {
                id: next_id,
                kind,
                col,
                row,
                cols,
                rows,
            });
            next_id += 1;
        }
        Self { tiles, next_id }
    }

    #[must_use]
    pub fn contains_kind(&self, kind: ControlKind) -> bool {
        self.tiles.iter().any(|tile| tile.kind == kind)
    }

    #[must_use]
    pub fn tile(&self, id: u32) -> Option<&ControlTile> {
        self.tiles.iter().find(|tile| tile.id == id)
    }

    #[must_use]
    pub fn row_count(&self) -> u8 {
        self.tiles
            .iter()
            .map(|tile| tile.row.saturating_add(tile.rows))
            .max()
            .unwrap_or(1)
            .max(1)
    }

    pub fn add_control(&mut self, kind: ControlKind) -> Option<u32> {
        if self.contains_kind(kind) || self.tiles.len() >= CONTROL_GRID_MAX_TILES {
            return None;
        }
        let size = kind.default_size();
        let (col, row) = self.first_free_slot(size.cols, size.rows);
        let id = self.next_id;
        self.next_id += 1;
        self.tiles.push(ControlTile {
            id,
            kind,
            col,
            row,
            cols: size.cols,
            rows: size.rows,
        });
        self.resolve_collisions(Some(id));
        Some(id)
    }

    pub fn remove_tile(&mut self, id: u32) -> bool {
        let before = self.tiles.len();
        self.tiles.retain(|tile| tile.id != id);
        self.tiles.len() != before
    }

    pub fn resize_tile(&mut self, id: u32, desired_cols: u8, desired_rows: u8) -> bool {
        let Some(index) = self.tiles.iter().position(|tile| tile.id == id) else {
            return false;
        };
        let snapped = self.tiles[index].kind.snap_size(desired_cols, desired_rows);
        let max_col = CONTROL_GRID_COLUMNS.saturating_sub(snapped.cols);
        let next_col = self.tiles[index].col.min(max_col);
        if self.tiles[index].cols == snapped.cols
            && self.tiles[index].rows == snapped.rows
            && self.tiles[index].col == next_col
        {
            return false;
        }
        self.tiles[index].cols = snapped.cols;
        self.tiles[index].rows = snapped.rows;
        self.tiles[index].col = next_col;
        self.resolve_collisions(Some(id));
        true
    }

    pub fn cycle_tile_size(&mut self, id: u32) -> bool {
        let Some(tile) = self.tile(id).copied() else {
            return false;
        };
        let next = tile.kind.next_size(tile.size());
        self.resize_tile(id, next.cols, next.rows)
    }

    pub fn move_tile(&mut self, id: u32, desired_col: u8, desired_row: u8) -> bool {
        let Some(index) = self.tiles.iter().position(|tile| tile.id == id) else {
            return false;
        };
        let moved = self.tiles[index];
        let clamped_col = desired_col.min(CONTROL_GRID_COLUMNS.saturating_sub(moved.cols));
        let clamped_row = desired_row.min(CONTROL_GRID_MAX_ROWS.saturating_sub(moved.rows));
        if moved.col == clamped_col && moved.row == clamped_row {
            return false;
        }
        let candidate = ControlTile {
            col: clamped_col,
            row: clamped_row,
            ..moved
        };
        let colliding: Vec<usize> = self
            .tiles
            .iter()
            .enumerate()
            .filter_map(|(idx, other)| (idx != index && candidate.overlaps(other)).then_some(idx))
            .collect();
        if colliding.len() == 1 {
            let other_idx = colliding[0];
            let other = self.tiles[other_idx];
            if other.cols == moved.cols && other.rows == moved.rows {
                self.tiles[index].col = other.col;
                self.tiles[index].row = other.row;
                self.tiles[other_idx].col = moved.col;
                self.tiles[other_idx].row = moved.row;
                return true;
            }
        }
        self.tiles[index].col = clamped_col;
        self.tiles[index].row = clamped_row;
        self.resolve_collisions(Some(id));
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default_ipad_grid();
    }

    #[must_use]
    pub fn is_future_version(contents: &str) -> bool {
        for line in contents.lines() {
            if let Some((key, value)) = line.trim().split_once('=')
                && key.trim().eq_ignore_ascii_case("version")
                && let Ok(ver) = value.trim().parse::<u32>()
            {
                return ver > 1;
            }
        }
        false
    }

    #[must_use]
    pub fn parse_config(contents: &str) -> Self {
        let mut tiles = Vec::new();
        let mut next_id = 1u32;
        for line in contents.lines() {
            let trimmed = line.trim();
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            if !key.trim().eq_ignore_ascii_case("tile") {
                continue;
            }
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            if parts.len() != 5 {
                continue;
            }
            let Some(kind) = ControlKind::from_key(parts[0]) else {
                continue;
            };
            if tiles
                .iter()
                .any(|existing: &ControlTile| existing.kind == kind)
            {
                continue;
            }
            let (Ok(col), Ok(row), Ok(cols), Ok(rows)) = (
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
                parts[3].parse::<u8>(),
                parts[4].parse::<u8>(),
            ) else {
                continue;
            };
            let snapped = kind.snap_size(cols, rows);
            let clamped_col = col.min(CONTROL_GRID_COLUMNS.saturating_sub(snapped.cols));
            let clamped_row = row.min(CONTROL_GRID_MAX_ROWS.saturating_sub(snapped.rows));
            tiles.push(ControlTile {
                id: next_id,
                kind,
                col: clamped_col,
                row: clamped_row,
                cols: snapped.cols,
                rows: snapped.rows,
            });
            next_id += 1;
            if tiles.len() >= CONTROL_GRID_MAX_TILES {
                break;
            }
        }
        if tiles.is_empty() {
            return Self::default_ipad_grid();
        }
        let mut grid = Self { tiles, next_id };
        grid.resolve_collisions(None);
        grid
    }

    #[must_use]
    pub fn format_config(&self) -> String {
        let mut out = String::from("version=1\n");
        let mut sorted = self.tiles.clone();
        sorted.sort_by_key(|tile| (tile.row, tile.col, tile.id));
        for tile in sorted {
            let _ = writeln!(
                out,
                "tile={},{},{},{},{}",
                tile.kind.key(),
                tile.col,
                tile.row,
                tile.cols,
                tile.rows
            );
        }
        out
    }

    fn first_free_slot(&self, cols: u8, rows: u8) -> (u8, u8) {
        let max_col = CONTROL_GRID_COLUMNS.saturating_sub(cols);
        for row in 0..CONTROL_GRID_MAX_ROWS {
            for col in 0..=max_col {
                let candidate = ControlTile {
                    id: 0,
                    kind: ControlKind::Settings,
                    col,
                    row,
                    cols,
                    rows,
                };
                if self.tiles.iter().all(|other| !candidate.overlaps(other)) {
                    return (col, row);
                }
            }
        }
        (0, self.row_count())
    }

    fn resolve_collisions(&mut self, pinned_id: Option<u32>) {
        let mut order: Vec<usize> = (0..self.tiles.len()).collect();
        order.sort_by_key(|&idx| {
            let tile = self.tiles[idx];
            let is_pinned = pinned_id == Some(tile.id);
            (u8::from(!is_pinned), tile.row, tile.col, tile.id)
        });

        let mut placed: Vec<ControlTile> = Vec::with_capacity(self.tiles.len());
        for idx in order {
            let mut tile = self.tiles[idx];
            tile.col = tile.col.min(CONTROL_GRID_COLUMNS.saturating_sub(tile.cols));
            if pinned_id == Some(tile.id) {
                placed.push(tile);
                continue;
            }
            while placed.iter().any(|other| tile.overlaps(other)) {
                let next_row = placed
                    .iter()
                    .filter(|other| tile.overlaps(other))
                    .map(|other| other.row.saturating_add(other.rows))
                    .max()
                    .unwrap_or_else(|| tile.row.saturating_add(1));
                if next_row <= tile.row {
                    tile.row = tile.row.saturating_add(1);
                } else {
                    tile.row = next_row;
                }
                if tile.row >= CONTROL_GRID_MAX_ROWS.saturating_add(8) {
                    break;
                }
            }
            self.tiles[idx] = tile;
            placed.push(tile);
        }
        self.tiles.sort_by_key(|tile| (tile.row, tile.col, tile.id));
    }
}
