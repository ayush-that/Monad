//! iPod navigation state.

use dioxus::prelude::*;

/// iPod color themes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColorTheme {
    #[default]
    Silver,
    Blue,
    Yellow,
    Pink,
    Red,
}

impl ColorTheme {
    /// Get all available themes.
    pub const fn all() -> &'static [ColorTheme] {
        &[
            ColorTheme::Silver,
            ColorTheme::Blue,
            ColorTheme::Yellow,
            ColorTheme::Pink,
            ColorTheme::Red,
        ]
    }

    /// Get display name.
    pub const fn name(self) -> &'static str {
        match self {
            ColorTheme::Silver => "Silver",
            ColorTheme::Blue => "Blue",
            ColorTheme::Yellow => "Yellow",
            ColorTheme::Pink => "Pink",
            ColorTheme::Red => "Red",
        }
    }

    /// Get CSS class name for this theme.
    pub const fn css_class(self) -> &'static str {
        match self {
            ColorTheme::Silver => "theme-silver",
            ColorTheme::Blue => "theme-blue",
            ColorTheme::Yellow => "theme-yellow",
            ColorTheme::Pink => "theme-pink",
            ColorTheme::Red => "theme-red",
        }
    }
}

/// iPod screen states.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IPodScreen {
    /// Now playing screen (shows current track).
    #[default]
    NowPlaying,
    /// Main menu.
    Menu,
    /// Search screen.
    Search,
    /// Settings screen.
    Settings,
}

/// Menu item definition.
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: &'static str,
    pub target: IPodScreen,
}

impl IPodScreen {
    /// Get menu items for this screen.
    pub fn menu_items(self) -> Vec<MenuItem> {
        match self {
            IPodScreen::Menu => vec![
                MenuItem {
                    label: "Now Playing",
                    target: IPodScreen::NowPlaying,
                },
                MenuItem {
                    label: "Search",
                    target: IPodScreen::Search,
                },
                MenuItem {
                    label: "Settings",
                    target: IPodScreen::Settings,
                },
            ],
            _ => vec![],
        }
    }

    /// Get the title for this screen.
    pub const fn title(self) -> &'static str {
        match self {
            IPodScreen::NowPlaying => "Now Playing",
            IPodScreen::Menu => "iPod",
            IPodScreen::Search => "Search",
            IPodScreen::Settings => "Settings",
        }
    }
}

/// iPod navigation state.
#[derive(Clone)]
pub struct IPodState {
    /// Current screen.
    pub screen: Signal<IPodScreen>,
    /// Selected menu index per screen.
    pub menu_index: Signal<usize>,
    /// Screen history for back navigation.
    pub history: Signal<Vec<IPodScreen>>,
    /// Current color theme.
    pub theme: Signal<ColorTheme>,
}

impl IPodState {
    /// Create new iPod state.
    pub fn new() -> Self {
        Self {
            screen: Signal::new(IPodScreen::NowPlaying),
            menu_index: Signal::new(0),
            history: Signal::new(Vec::new()),
            theme: Signal::new(ColorTheme::default()),
        }
    }

    /// Navigate to a screen.
    pub fn navigate(&mut self, screen: IPodScreen) {
        let current = *self.screen.read();
        self.history.write().push(current);
        *self.screen.write() = screen;
        *self.menu_index.write() = 0;
    }

    /// Go back to previous screen.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.history.write().pop() {
            *self.screen.write() = prev;
            *self.menu_index.write() = 0;
        }
    }

    /// Move selection up in menu.
    pub fn select_previous(&mut self) {
        let current = *self.menu_index.read();
        if current > 0 {
            *self.menu_index.write() = current - 1;
        }
    }

    /// Move selection down in menu.
    pub fn select_next(&mut self, max_items: usize) {
        let current = *self.menu_index.read();
        if current < max_items.saturating_sub(1) {
            *self.menu_index.write() = current + 1;
        }
    }

    /// Select current menu item.
    pub fn select(&mut self) {
        let screen = *self.screen.read();
        let items = screen.menu_items();
        let index = *self.menu_index.read();

        if let Some(item) = items.get(index) {
            self.navigate(item.target);
        }
    }
}

impl Default for IPodState {
    fn default() -> Self {
        Self::new()
    }
}
