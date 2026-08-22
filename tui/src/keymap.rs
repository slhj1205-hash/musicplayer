use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    TogglePanel,
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    JumpTop,
    JumpBottom,
    JumpToCurrent,
    Activate,
    TogglePlayback,
    NextOrJump,
    PrevTrack,
    QueueNext,
    OpenSongModal,
    OpenMetadataEditModal,
    OpenYoutubeModal,
    RemoveFromPlaylist,
    ChangeDirectory,
    ToggleSearch,
    CycleCategory(Direction),
    CycleSort(Direction),
    CyclePlaylistDisplayMode,
    Shuffle,
    Unshuffle,
    VolumeUp,
    VolumeDown,
    Quit,
    ToggleHelp,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Global,
    Library,
    Playlists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forwards,
    Backwards,
}

pub struct Binding {
    pub keys: &'static [(KeyCode, KeyModifiers)],
    pub action: Action,
    pub display: &'static str,
    pub desc: &'static str,
    pub section: Section,
    pub dispatch: bool,
}

const NONE: KeyModifiers = KeyModifiers::NONE;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const ALT: KeyModifiers = KeyModifiers::ALT;

pub const BINDINGS: &[Binding] = &[
    Binding { keys: &[(KeyCode::Tab, NONE)], action: Action::TogglePanel,
        display: "<Tab>", desc: "Switch between Library / Playlists", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('j'), NONE), (KeyCode::Down, NONE)], action: Action::MoveDown,
        display: "<j>/<↓>", desc: "Move selection", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('k'), NONE), (KeyCode::Up, NONE)], action: Action::MoveUp,
        display: "<k>/<↑>", desc: "Move selection", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('d'), CTRL)], action: Action::PageDown,
        display: "<Ctrl+d>", desc: "Jump a page down / up", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('u'), CTRL)], action: Action::PageUp,
        display: "<Ctrl+u>", desc: "Jump a page down / up", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('g'), NONE), (KeyCode::Home, NONE)], action: Action::JumpTop,
        display: "<g>/<Home>", desc: "Jump to top / bottom", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('G'), NONE), (KeyCode::End, NONE)], action: Action::JumpBottom,
        display: "<Shift+G>/<End>", desc: "Jump to top / bottom", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('c'), NONE)], action: Action::JumpToCurrent,
        display: "<c>", desc: "Jump to now playing", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Enter, NONE)], action: Action::Activate,
        display: "<Enter>", desc: "Play selected song / open selected playlist", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char(' '), NONE)], action: Action::TogglePlayback,
        display: "<Space>", desc: "Pause / resume", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('n'), NONE)], action: Action::NextOrJump,
        display: "<n>", desc: "Next track", section: Section::Global, dispatch: true },
    Binding { keys: &[], action: Action::None,
        display: "<1-9> then <n>", desc: "Jump to Nth song in Up Next (<Esc> cancels)", section: Section::Global, dispatch: false },
    Binding { keys: &[(KeyCode::Char('b'), NONE)], action: Action::PrevTrack,
        display: "<b>", desc: "Previous track", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('a'), NONE)], action: Action::QueueNext,
        display: "<a>", desc: "Queue selected song next", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('s'), NONE)], action: Action::Shuffle,
        display: "<s>", desc: "Shuffle", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('u'), NONE)], action: Action::Unshuffle,
        display: "<u>", desc: "Un-shuffle", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('['), NONE), (KeyCode::Char('-'), NONE)], action: Action::VolumeDown,
        display: "<[>/<->", desc: "Volume down / up", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char(']'), NONE), (KeyCode::Char('='), NONE)], action: Action::VolumeUp,
        display: "<]>/<=>", desc: "Volume down / up", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('w'), NONE)], action: Action::OpenSongModal,
        display: "<w>", desc: "Song actions: add to / create playlist", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('E'), NONE)], action: Action::OpenMetadataEditModal,
        display: "<Shift+E>", desc: "Edit song metadata (title/artist/album/genre/track/date)", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('y'), NONE)], action: Action::OpenYoutubeModal,
        display: "<y>", desc: "Download audio from a YouTube URL", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('d'), ALT)], action: Action::ChangeDirectory,
        display: "<Alt+d>", desc: "Change directory (used by both Library and Playlists)", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('q'), NONE)], action: Action::Quit,
        display: "<q>", desc: "Quit (with confirmation)", section: Section::Global, dispatch: true },
    Binding { keys: &[], action: Action::None,
        display: "<Esc>", desc: "Quit (with confirmation)", section: Section::Global, dispatch: false },
    Binding { keys: &[(KeyCode::Char('?'), NONE)], action: Action::ToggleHelp,
        display: "<?>", desc: "Toggle this help", section: Section::Global, dispatch: true },
    Binding { keys: &[(KeyCode::Char('/'), NONE)], action: Action::ToggleSearch,
        display: "</>", desc: "Search the library (live filter)", section: Section::Library, dispatch: true },
    Binding { keys: &[(KeyCode::Char('o'), NONE)], action: Action::CycleCategory(Direction::Forwards),
        display: "<o>", desc: "Cycle library category (grouping)", section: Section::Library, dispatch: true },
    Binding { keys: &[(KeyCode::Char('O'), NONE)], action: Action::CycleCategory(Direction::Backwards),
        display: "<Shift+O>", desc: "Cycle library category (grouping)", section: Section::Library, dispatch: true },
    Binding { keys: &[(KeyCode::Char('p'), NONE)], action: Action::CycleSort(Direction::Forwards),
        display: "<p>", desc: "Cycle library sort (order within group)", section: Section::Library, dispatch: true },
    Binding { keys: &[(KeyCode::Char('P'), NONE)], action: Action::CycleSort(Direction::Backwards),
        display: "<Shift+P>", desc: "Cycle library sort (order within group)", section: Section::Library, dispatch: true },
    Binding { keys: &[(KeyCode::Char('m'), NONE)], action: Action::CyclePlaylistDisplayMode,
        display: "<m>", desc: "Cycle playlist display: hidden / count / names", section: Section::Library, dispatch: true },
    Binding { keys: &[], action: Action::None,
        display: "</>", desc: "Search by name / within the open playlist (live filter)", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<Enter>", desc: "Open playlist / play selected song within it", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<Esc>", desc: "Back to playlist browser", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<o>", desc: "Cycle category within the open playlist", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<Shift+O>", desc: "Cycle category within the open playlist", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<p>", desc: "Cycle sort within the open playlist", section: Section::Playlists, dispatch: false },
    Binding { keys: &[], action: Action::None,
        display: "<Shift+P>", desc: "Cycle sort within the open playlist", section: Section::Playlists, dispatch: false },
    Binding { keys: &[(KeyCode::Char('r'), NONE)], action: Action::RemoveFromPlaylist,
        display: "<r>", desc: "Remove selected song from playlist (confirm)", section: Section::Playlists, dispatch: true },
];

pub const FOOTER_HINT: &str =
    "<↑>/<↓> select · <Enter> play · <Space> pause · <Tab> playlists · <?> more · <q> quit";

pub fn lookup(key: KeyEvent) -> Option<Action> {
    BINDINGS
        .iter()
        .filter(|b| b.dispatch)
        .find(|b| b.keys.iter().any(|&(code, mods)| code == key.code && key.modifiers.contains(mods)))
        .map(|b| b.action)
}

pub fn display_for(action: Action) -> &'static str {
    BINDINGS.iter().find(|b| b.action == action).map(|b| b.display).unwrap_or("")
}

pub fn help_rows(section: Section) -> Vec<(String, &'static str)> {
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    for b in BINDINGS.iter().filter(|b| b.section == section) {
        if let Some(last) = rows.last_mut() && last.1 == b.desc {
            last.0.push_str(" / ");
            last.0.push_str(b.display);
            continue;
        }
        rows.push((b.display.to_string(), b.desc));
    }
    rows
}
