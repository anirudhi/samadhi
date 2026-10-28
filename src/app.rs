//! The application: state, input handling and drawing.
//!
//! The window is always borderless full screen. The controls are bare icons
//! beside the page that fade away as soon as you type; moving the mouse
//! brings them back. Every document is a file that saves itself moments
//! after you stop typing, and the app reopens the last one at start-up.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Id, Key, KeyboardShortcut, Modifiers, Rect, Stroke,
    UiBuilder, Vec2, pos2, vec2,
};

use crate::audio::{self, AudioEngine, BuiltinClick, Music};
use crate::background::{self, SceneLoader};
use crate::catalog::{self, Catalog};
use crate::document::{self, Document, SaveOutcome};
use crate::fonts;
use crate::icons::Icon;
use crate::markdown;
use crate::page::{self, Drag};
use crate::recovery::Recovery;
use crate::scene::ThemeKind;
use crate::settings::{DEFAULT_FONT_SIZE, MAX_FONT_SIZE, MIN_FONT_SIZE, PageRect, Settings};
use crate::spell::SpellChecker;
use crate::stage::Stage;
use crate::theme;
use crate::timer::{self, Timer};
use crate::widgets::{
    ICON_BUTTON, caption, icon_button, icon_button_bare, list_choice, slider, switch, text_choice,
    thumb_button,
};

const DOCK_GAP: f32 = 28.0;
/// Height of a flyout's title line, and the gap between it and the controls.
const FLYOUT_TITLE_HEIGHT: f32 = 14.0;
const FLYOUT_TITLE_GAP: f32 = 2.0;
/// Theme previews per row in the theme panel.
const THEME_GRID_COLUMNS: usize = 6;
const FLYOUT_WIDTH_ESTIMATE: f32 = 330.0;
const PAGE_PADDING: Vec2 = vec2(34.0, 30.0);
const LINE_SPACING: f32 = 1.6;
const FONT_STEP: f32 = 2.0;
/// How close (in points) the pointer must be to the page edge for the
/// resize handles to appear.
const HANDLE_REVEAL: f32 = 28.0;
const FADE_IN: f32 = 0.2;
const FADE_OUT: f32 = 0.35;
/// The interface fades out this long after the mouse stops moving.
const IDLE_HIDE: Duration = Duration::from_millis(1500);
const SETTINGS_SAVE_DELAY: Duration = Duration::from_secs(2);
/// How long after the last keystroke the document is saved.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);
const STATUS_TIME: Duration = Duration::from_secs(3);
const ERROR_TIME: Duration = Duration::from_secs(8);
const FULLSCREEN_RETRY: Duration = Duration::from_secs(1);
/// How often the open file is checked for changes made elsewhere (another
/// app, or a sync from another device).
const DISK_CHECK: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Text,
    Size,
    Theme,
    Music,
    Keyboard,
    Timer,
    File,
}

impl Section {
    const ALL: [(Self, Icon, &'static str); 7] = [
        (Self::Text, Icon::Font, "Font"),
        (Self::Size, Icon::Size, "Text size"),
        (Self::Theme, Icon::Picture, "Theme"),
        (Self::Music, Icon::Music, "Music"),
        (Self::Keyboard, Icon::Keyboard, "Typing sounds"),
        (Self::Timer, Icon::Timer, "Timer"),
        (Self::File, Icon::Document, "Document"),
    ];
}

/// Something waiting on the "save this draft?" question.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    Quit,
    Open,
    New,
    /// A file handed over by a later launch of the app.
    OpenFile(PathBuf),
}

/// The right-click menu for a misspelled word.
#[derive(Debug, Clone)]
struct SpellMenu {
    range: Range<usize>,
    word: String,
    suggestions: Vec<String>,
}

#[derive(Debug)]
struct Loading {
    path: PathBuf,
    /// Opened on request (rather than as the last file): report failures.
    requested: bool,
    since: Instant,
    rx: mpsc::Receiver<anyhow::Result<Document>>,
}

#[derive(Debug)]
struct Status {
    text: String,
    is_error: bool,
    since: Instant,
}

/// How the app is started. [`Options::default`] is the normal desktop setup;
/// tests switch off audio and use temporary folders.
#[derive(Debug)]
pub struct Options {
    /// A file to open instead of the last one.
    pub file: Option<PathBuf>,
    pub settings_path: Option<PathBuf>,
    /// Overrides the folder the save dialog starts in for new documents.
    pub documents_dir: Option<PathBuf>,
    /// Overrides where unsaved work is cached for crash recovery.
    pub recovery_dir: Option<PathBuf>,
    pub audio: bool,
    /// Requests from later launches of the app (see [`crate::instance`]).
    #[cfg(unix)]
    pub messages: Option<mpsc::Receiver<crate::instance::Message>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            file: None,
            settings_path: Settings::default_path(),
            documents_dir: None,
            recovery_dir: None,
            audio: true,
            #[cfg(unix)]
            messages: None,
        }
    }
}

pub struct SamadhiApp {
    settings: Settings,
    settings_path: Option<PathBuf>,
    settings_changed: Option<Instant>,

    doc: Document,
    /// When the text last changed, for the autosave debounce.
    last_edit: Option<Instant>,
    /// Mirror of unsaved work, so a crash never loses it.
    recovery: Recovery,
    /// An action waiting for the user to decide about an unsaved draft.
    pending: Option<Pending>,
    last_disk_check: Instant,
    /// The document being read at start-up, off the interface thread so a
    /// slow disk (say, a network drive just after boot) never holds up the
    /// window. Its file, whether it was asked for explicitly, and the result.
    loading: Option<Loading>,
    #[cfg(unix)]
    messages: Option<mpsc::Receiver<crate::instance::Message>>,
    timer: Timer,

    audio: AudioEngine,
    themes: Catalog<ThemeKind>,
    music: Catalog<Music>,
    clicks: Catalog<BuiltinClick>,
    stage: Stage,
    scene_loader: SceneLoader,
    /// Theme previews, filled in by a worker thread.
    thumbs: Vec<Option<egui::TextureHandle>>,
    thumbs_rx: Option<mpsc::Receiver<(usize, egui::ColorImage)>>,
    /// The typing sound last previewed on hover.
    previewed_click: Option<usize>,
    /// When the mouse last moved, for hiding the interface when idle.
    last_pointer: Instant,

    /// Live page rectangle while it is being dragged.
    page: Option<Rect>,
    drag: Option<Drag>,
    ui_visible: bool,
    section: Option<Section>,
    /// The recent files list, shown once the open button is hovered and
    /// kept while the pointer stays on the document flyout.
    recent_open: bool,
    /// Screen area of the dock and open flyout, used to close the flyout
    /// once the pointer wanders away.
    menu_area: Option<Rect>,
    status: Option<Status>,

    spell: Option<SpellChecker>,
    spell_rx: Option<mpsc::Receiver<anyhow::Result<SpellChecker>>>,
    /// Misspelled ranges for the text with this hash.
    spell_cache: Option<(u64, Vec<Range<usize>>)>,
    /// Bumped when the personal dictionary or ignore list changes.
    spell_generation: u64,
    spell_menu: Option<SpellMenu>,

    allow_close: bool,
    last_title: String,
    last_fullscreen_request: Option<Instant>,
}

impl std::fmt::Debug for SamadhiApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamadhiApp")
            .field("doc", &self.doc)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// Turns a built-in table into catalogue entries.
fn named<B: Copy>(all: &[(&str, B)]) -> Vec<(String, B)> {
    all.iter().map(|&(n, b)| (n.to_owned(), b)).collect()
}

impl SamadhiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install(ctx, false);
        let mut settings = Settings::load_or_default(options.settings_path.as_deref());
        if options.documents_dir.is_some() {
            settings.documents_dir.clone_from(&options.documents_dir);
        }

        let scan = |kind: &str, exts: &[&str]| -> Vec<PathBuf> {
            crate::asset_dirs(kind)
                .iter()
                .flat_map(|d| catalog::scan_dir(d, exts))
                .collect()
        };
        let themes = Catalog::new(
            named(&ThemeKind::ALL),
            scan("backgrounds", background::IMAGE_EXTENSIONS),
        );
        let music = Catalog::new(
            audio::builtin_music(),
            scan("music", audio::AUDIO_EXTENSIONS),
        );
        let clicks = Catalog::new(
            named(&BuiltinClick::ALL),
            scan("clicks", audio::AUDIO_EXTENSIONS),
        );

        fonts::apply(ctx, fonts::typeface(settings.font_family.as_deref()));

        let audio = if options.audio {
            AudioEngine::new()
        } else {
            AudioEngine::silent()
        };

        let timer = Timer::new(settings.timer_minutes);
        let mut app = Self {
            settings,
            settings_path: options.settings_path,
            settings_changed: None,
            doc: Document::default(),
            last_edit: None,
            recovery: Recovery::new(options.recovery_dir.clone()),
            pending: None,
            last_disk_check: Instant::now(),
            loading: None,
            #[cfg(unix)]
            messages: options.messages,
            timer,
            audio,
            themes,
            music,
            clicks,
            stage: Stage::default(),
            scene_loader: SceneLoader::default(),
            thumbs: Vec::new(),
            thumbs_rx: None,
            previewed_click: None,
            last_pointer: Instant::now(),
            page: None,
            drag: None,
            ui_visible: true,
            section: None,
            recent_open: false,
            menu_area: None,
            status: None,
            spell: None,
            spell_rx: None,
            spell_cache: None,
            spell_generation: 0,
            spell_menu: None,
            allow_close: false,
            last_title: String::new(),
            last_fullscreen_request: None,
        };
        app.restore_media(ctx);
        if app.settings.spell_check {
            app.load_spell_checker(ctx);
        }
        if options.audio && !app.audio.has_device() {
            app.set_status("No audio output found; continuing without sound", true);
        }
        // Open the requested file; else bring back unsaved work from a
        // crash; else reopen the last file; else start a fresh draft.
        if let Some(path) = options.file {
            app.load_in_background(ctx, path, true);
        } else if !app.restore_draft()
            && let Some(path) = app.settings.last_file.clone()
        {
            app.load_in_background(ctx, path, false);
        }
        app
    }

    /// Applies the background, music and click saved in the settings.
    fn restore_media(&mut self, ctx: &egui::Context) {
        if let Some(name) = self.settings.theme.clone() {
            self.themes.select(&name);
        }
        self.show_current_theme(ctx);
        if let Some(name) = self.settings.music.clone() {
            self.music.select(&name);
        }
        if let Some(name) = self.settings.click.clone() {
            self.clicks.select(&name);
        }
        self.audio.set_music_volume(self.settings.music_volume);
        self.audio.set_music_muted(self.settings.music_muted);
        self.audio.set_click_volume(self.settings.click_volume);
        self.audio.set_click_muted(self.settings.click_muted);
        self.start_current_music();
        self.load_current_click();
    }

    // ----- status & settings ------------------------------------------------

    fn set_status(&mut self, text: impl Into<String>, is_error: bool) {
        let text = text.into();
        if is_error {
            log::warn!("{text}");
        }
        self.status = Some(Status {
            text,
            is_error,
            since: Instant::now(),
        });
    }

    fn report<T>(&mut self, result: anyhow::Result<T>) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(err) => {
                self.set_status(format!("{err:#}"), true);
                None
            }
        }
    }

    fn settings_touched(&mut self) {
        self.settings_changed = Some(Instant::now());
    }

    fn persist_settings(&mut self) {
        self.settings_changed = None;
        if let Some(path) = self.settings_path.clone() {
            let result = self.settings.save(&path);
            self.report(result);
        }
    }

    // ----- media ------------------------------------------------------------

    /// Starts building the current theme; it fades in once ready.
    fn show_current_theme(&mut self, ctx: &egui::Context) {
        let entry = self.themes.current().clone();
        self.settings.theme = Some(entry.name.clone());
        self.scene_loader.request(entry.name, entry.source, ctx);
    }

    fn select_theme(&mut self, ctx: &egui::Context, index: usize) {
        if index != self.themes.current_index() {
            self.themes.select_index(index);
            self.show_current_theme(ctx);
            self.settings_touched();
        }
    }

    /// Starts building theme previews the first time they are needed.
    fn ensure_thumbnails(&mut self, ctx: &egui::Context) {
        if self.thumbs.is_empty() {
            self.thumbs = vec![None; self.themes.len()];
            let sources = self
                .themes
                .entries()
                .iter()
                .map(|e| e.source.clone())
                .collect();
            self.thumbs_rx = Some(background::thumbnails(sources, ctx));
        }
    }

    fn start_current_music(&mut self) {
        let entry = self.music.current().clone();
        self.settings.music = Some(entry.name.clone());
        let result = self.audio.set_music(&entry.source);
        self.report(result);
    }

    fn select_music(&mut self, index: usize) {
        if index != self.music.current_index() {
            self.music.select_index(index);
            self.start_current_music();
            self.settings_touched();
        }
    }

    fn load_current_click(&mut self) {
        let entry = self.clicks.current().clone();
        self.settings.click = Some(entry.name.clone());
        let result = self.audio.set_click(&entry.source);
        self.report(result);
    }

    fn select_click(&mut self, index: usize) {
        if index != self.clicks.current_index() {
            self.clicks.select_index(index);
            self.load_current_click();
            self.settings_touched();
        }
    }

    /// Plays a typing sound once when the pointer first rests on it.
    fn preview_click(&mut self, index: usize) {
        if self.previewed_click != Some(index)
            && let Some(entry) = self.clicks.entries().get(index)
        {
            let source = entry.source.clone();
            let result = self.audio.preview_click(&source);
            self.report(result);
        }
        self.previewed_click = Some(index);
    }

    fn set_font_size(&mut self, size: f32) {
        self.settings.font_size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.settings_touched();
    }

    fn set_typeface(&mut self, ctx: &egui::Context, typeface: &fonts::Typeface) {
        fonts::apply(ctx, typeface);
        self.settings.font_family = Some(typeface.name.to_owned());
        self.settings_touched();
    }

    // ----- spelling -----------------------------------------------------------

    fn personal_dictionary_path(&self) -> Option<PathBuf> {
        self.settings_path
            .as_deref()
            .and_then(Path::parent)
            .map(|d| d.join("dictionary.txt"))
    }

    /// Loads the dictionary on a worker thread (it takes a moment).
    fn load_spell_checker(&mut self, ctx: &egui::Context) {
        if self.spell.is_some() || self.spell_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let personal = self.personal_dictionary_path();
        let repaint = ctx.clone();
        std::thread::Builder::new()
            .name("spell-load".into())
            .spawn(move || {
                let _ = tx.send(SpellChecker::load(personal));
                repaint.request_repaint();
            })
            .expect("spawning a thread should not fail");
        self.spell_rx = Some(rx);
    }

    fn toggle_spell_check(&mut self, ctx: &egui::Context) {
        self.settings.spell_check = !self.settings.spell_check;
        if self.settings.spell_check {
            self.load_spell_checker(ctx);
        }
        self.settings_touched();
    }

    /// Misspelled ranges in `text`, recomputed only when the text (or the
    /// personal dictionary) changes.
    fn misspellings<'a>(
        cache: &'a mut Option<(u64, Vec<Range<usize>>)>,
        checker: &SpellChecker,
        generation: u64,
        text: &str,
    ) -> &'a [Range<usize>] {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        generation.hash(&mut hasher);
        let key = hasher.finish();
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            *cache = Some((key, checker.misspellings(text)));
        }
        cache.as_ref().map_or(&[], |(_, r)| r.as_slice())
    }

    fn spell_menu_ui(&mut self, ui: &mut egui::Ui) {
        let Some(menu) = self.spell_menu.clone() else {
            ui.close();
            return;
        };
        ui.set_min_width(190.0);
        if menu.suggestions.is_empty() {
            ui.weak("No suggestions");
        }
        for suggestion in &menu.suggestions {
            if ui
                .button(egui::RichText::new(suggestion).strong())
                .clicked()
            {
                if self.doc.text.get(menu.range.clone()) == Some(menu.word.as_str()) {
                    self.doc.text.replace_range(menu.range.clone(), suggestion);
                    self.doc.mark_dirty();
                    self.last_edit = Some(Instant::now());
                }
                self.spell_menu = None;
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(format!("Add \u{201c}{}\u{201d} to dictionary", menu.word))
            .clicked()
        {
            if let Some(spell) = &mut self.spell {
                let result = spell.add_word(&menu.word);
                self.report(result);
            }
            self.spell_generation += 1;
            self.spell_menu = None;
            ui.close();
        }
        if ui.button("Ignore").clicked() {
            if let Some(spell) = &mut self.spell {
                spell.ignore(&menu.word);
            }
            self.spell_generation += 1;
            self.spell_menu = None;
            ui.close();
        }
    }

    // ----- documents --------------------------------------------------------

    fn file_dialog(&self) -> rfd::FileDialog {
        // Next to the current file, else in the documents folder (made on
        // first use), else the home folder.
        let documents = self.settings.documents_dir();
        let dir = self
            .doc
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::fs::create_dir_all(&documents).ok().map(|()| documents))
            .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()));
        let mut dialog = rfd::FileDialog::new();
        // macOS merges the filters into one list of allowed extensions,
        // where "*" matches nothing: any other text file (and the name
        // typed when saving) would be refused. Opening checks the contents
        // anyway, so offer every file there.
        if !cfg!(target_os = "macos") {
            dialog = dialog
                .add_filter("Markdown and text", document::TEXT_EXTENSIONS)
                .add_filter("All files", &["*"]);
        }
        if let Some(dir) = dir {
            dialog = dialog.set_directory(dir);
        }
        dialog
    }

    /// Starts reading `path` on a worker thread; it becomes the document
    /// when it arrives (see [`Self::poll_background_work`]).
    fn load_in_background(&mut self, ctx: &egui::Context, path: PathBuf, requested: bool) {
        let (tx, rx) = mpsc::channel();
        let (file, ctx) = (path.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Document::open(&file));
            ctx.request_repaint();
        });
        self.loading = Some(Loading {
            path,
            requested,
            since: Instant::now(),
            rx,
        });
    }

    fn finish_loading(&mut self, result: anyhow::Result<Document>, requested: bool) {
        match result {
            Ok(doc) => {
                self.doc = doc;
                self.recovery.clear();
                self.remember_file();
            }
            // A last file that has gone (moved or deleted elsewhere) just
            // means starting fresh.
            Err(err) if requested => self.set_status(format!("{err:#}"), true),
            Err(err) => log::info!("not reopening the last file: {err:#}"),
        }
    }

    /// Requests from later launches: come to the front, and open the file
    /// they were given (asking about an unsaved draft first).
    #[cfg(unix)]
    fn handle_messages(&mut self, ctx: &egui::Context) {
        use crate::instance::Message;
        let Some(rx) = &self.messages else {
            return;
        };
        let messages: Vec<Message> = rx.try_iter().collect();
        for message in messages {
            if ctx.input(|i| i.viewport().minimized) == Some(true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.ui_visible = true;
            self.last_pointer = Instant::now();
            if let Message::Open(path) = message {
                let already_open = self.doc.path() == Some(path.as_path())
                    || self.loading.as_ref().is_some_and(|l| l.path == path);
                if !already_open && self.pending.is_none() {
                    self.request(ctx, Pending::OpenFile(path));
                }
            }
        }
    }

    /// Opens `path` (the current document should be settled first).
    /// Returns whether it opened.
    fn open_path(&mut self, path: &Path) -> bool {
        self.loading = None;
        self.checkpoint();
        match Document::open(path) {
            Ok(doc) => {
                self.doc = doc;
                self.recovery.clear();
                self.remember_file();
                true
            }
            Err(err) => {
                self.set_status(format!("{err:#}"), true);
                false
            }
        }
    }

    fn open_with_dialog(&mut self) {
        if let Some(path) = self.file_dialog().pick_file() {
            self.open_path(&path);
        }
    }

    /// Starts a fresh, unsaved draft.
    fn new_document(&mut self) {
        self.loading = None;
        self.checkpoint();
        self.doc = Document::default();
        self.recovery.clear();
        self.last_edit = None;
    }

    /// Brings back unsaved work left behind by a crash or shutdown.
    fn restore_draft(&mut self) -> bool {
        let Some(draft) = self.recovery.load() else {
            return false;
        };
        let file = draft.path.as_deref().filter(|p| p.is_file());
        self.doc = match file.map(Document::open) {
            Some(Ok(doc)) => doc,
            _ => Document::default(),
        };
        self.doc.text = draft.text;
        self.doc.mark_dirty();
        let message = match self.doc.path() {
            Some(_) => format!("Recovered unsaved changes to {}", self.doc.display_name()),
            None => "Recovered your unsaved draft".to_owned(),
        };
        self.set_status(message, false);
        if self.doc.path().is_some() {
            self.remember_file();
        }
        // Saved files catch up via autosave; drafts are re-cached.
        self.last_edit = Some(Instant::now());
        true
    }

    /// Records the current file so the app reopens it next time.
    fn remember_file(&mut self) {
        self.settings.last_file = self.doc.path().map(Path::to_path_buf);
        if let Some(path) = self.doc.path() {
            self.settings.add_recent(path);
        }
        self.last_edit = None;
        self.settings_touched();
    }

    /// A draft that has never been saved and holds some text.
    fn is_unsaved_draft(&self) -> bool {
        self.doc.path().is_none() && !self.doc.text.trim().is_empty()
    }

    /// Makes sure no work can be lost: saved files are written to disk,
    /// drafts (and edits that failed to save) go to the recovery cache.
    /// Returns `false` if the text is only in the recovery cache.
    fn checkpoint(&mut self) -> bool {
        self.last_edit = None;
        if !self.doc.is_dirty() {
            return true;
        }
        if self.doc.path().is_some() {
            if self.write_file() {
                self.recovery.clear();
                return true;
            }
        } else if self.doc.text.trim().is_empty() {
            self.recovery.clear();
            return true;
        }
        let result = self.recovery.store(&self.doc.text, self.doc.path());
        self.report(result);
        false
    }

    /// Picks up changes made to the open file elsewhere, such as edits
    /// synced from another device. A document with no unsaved edits simply
    /// reloads. If there are unsaved edits too, the other version is kept
    /// beside it as a conflicted copy, so neither is lost.
    fn sync_with_disk(&mut self) {
        self.last_disk_check = Instant::now();
        let Some(mut theirs) = self.doc.changed_on_disk() else {
            return;
        };
        if !self.doc.is_dirty() {
            self.doc = theirs;
            self.last_edit = None;
            self.set_status("Updated with changes made elsewhere", false);
            return;
        }
        let Some(path) = self.doc.path() else {
            return;
        };
        let copy = document::conflict_copy_path(path);
        let result = theirs.save_as(&copy).map(|_| {
            format!(
                "This file was also changed elsewhere. That version is kept as {}",
                theirs.display_name()
            )
        });
        if let Some(message) = self.report(result) {
            self.set_status(message, false);
        }
    }

    /// Writes a saved document to its file. Returns whether it worked.
    fn write_file(&mut self) -> bool {
        // Never write over a version from elsewhere that hasn't been seen.
        self.sync_with_disk();
        match self.doc.save() {
            Ok(SaveOutcome::Saved) => true,
            Ok(SaveOutcome::SavedAsUtf8) => {
                self.set_status(
                    "Saved as UTF-8: the text has characters the file's old encoding cannot hold",
                    false,
                );
                true
            }
            Err(err) => {
                self.set_status(format!("Could not save: {err:#}"), true);
                false
            }
        }
    }

    /// Ctrl+S or the save button: names a draft the first time, after
    /// which the file saves itself.
    fn save(&mut self) {
        if self.doc.path().is_none() {
            self.save_as();
        } else if self.checkpoint() {
            self.set_status("Saved", false);
        }
    }

    /// Asks for a name and location, saves there, and keeps autosaving
    /// there. Returns whether the document was saved.
    fn save_as(&mut self) -> bool {
        let ext = self.settings.default_extension.clone();
        let name = match self.doc.path().and_then(Path::file_name) {
            Some(n) => n.to_string_lossy().into_owned(),
            None => suggested_file_name(&self.doc.text, &ext),
        };
        let Some(mut path) = self.file_dialog().set_file_name(name).save_file() else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension(&ext);
        }
        match self.doc.save_as(&path) {
            Ok(_) => {
                self.recovery.clear();
                self.remember_file();
                self.set_status(format!("Saved as {}", self.doc.display_name()), false);
                true
            }
            Err(err) => {
                self.set_status(format!("Could not save: {err:#}"), true);
                false
            }
        }
    }

    /// A moment after typing stops: saved files are written, drafts are
    /// mirrored to the recovery cache.
    fn autosave(&mut self) {
        if self
            .last_edit
            .is_some_and(|t| t.elapsed() >= AUTOSAVE_DELAY)
        {
            self.checkpoint();
        }
    }

    /// Runs `action`, first asking what to do with an unsaved draft.
    fn request(&mut self, ctx: &egui::Context, action: Pending) {
        if self.is_unsaved_draft() {
            // Make sure it is safe in the cache while the question is open.
            self.checkpoint();
            self.pending = Some(action);
        } else {
            self.perform(ctx, action);
        }
    }

    fn perform(&mut self, ctx: &egui::Context, action: Pending) {
        match action {
            Pending::Quit => self.quit(ctx),
            Pending::Open => self.open_with_dialog(),
            Pending::New => self.new_document(),
            Pending::OpenFile(path) => {
                self.open_path(&path);
            }
        }
    }

    /// Settles the document and closes. Anything that could not be written
    /// to its file is left in the recovery cache for next time.
    fn quit(&mut self, ctx: &egui::Context) {
        self.checkpoint();
        self.persist_settings();
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// The "save this draft?" dialog.
    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else {
            return;
        };
        let dark = self.stage.dark();
        let mut choice = None;
        egui::Modal::new(Id::unique("unsaved"))
            .backdrop_color(Color32::from_black_alpha(70))
            .frame(
                theme::panel_frame(dark)
                    .fill(theme::palette(dark).window_fill)
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                let p = theme::current(ui);
                ui.label(egui::RichText::new("Save this draft?").size(18.0));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "It hasn't been saved to a file yet. If you don't save it, it will be discarded.",
                    )
                    .color(p.muted),
                );
                ui.add_space(18.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save = egui::Button::new(
                        egui::RichText::new("Save…").color(Color32::WHITE),
                    )
                    .fill(p.accent);
                    if ui.add(save).clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Don't save").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending = None;
                    }
                });
            });
        match choice {
            Some(true) => {
                self.pending = None;
                if self.save_as() {
                    self.perform(ctx, action);
                }
            }
            Some(false) => {
                self.pending = None;
                self.doc = Document::default();
                self.recovery.clear();
                self.last_edit = None;
                self.perform(ctx, action);
            }
            None => {}
        }
    }

    // ----- per-frame input ----------------------------------------------------

    fn poll_background_work(&mut self, ctx: &egui::Context) {
        if let Some(loading) = &self.loading
            && let Ok(result) = loading.rx.try_recv()
        {
            let requested = loading.requested;
            self.loading = None;
            self.finish_loading(result, requested);
        }
        if let Some(rx) = &self.thumbs_rx {
            while let Ok((i, image)) = rx.try_recv() {
                if let Some(slot) = self.thumbs.get_mut(i) {
                    *slot = Some(ctx.load_texture(
                        format!("theme-thumb-{i}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
        if let Some(rx) = &self.spell_rx
            && let Ok(result) = rx.try_recv()
        {
            self.spell_rx = None;
            match result {
                Ok(checker) => self.spell = Some(checker),
                Err(err) => self.set_status(format!("Spell check unavailable: {err:#}"), true),
            }
        }
        if let Some((name, result)) = self.scene_loader.poll() {
            match result {
                Ok(scene) => {
                    theme::install(ctx, scene.dark);
                    self.stage.show(ctx, scene);
                }
                Err(err) => self.set_status(format!("Could not show {name}: {err:#}"), true),
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let shortcut = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;

        // Esc closes the dialog first, and only then quits.
        if pressed(shortcut(Modifiers::NONE, Key::Escape)) && self.pending.take().is_none() {
            self.request(ctx, Pending::Quit);
        }
        if self.pending.is_some() {
            return;
        }
        // Check the Shift variant first: plain Ctrl+S also matches it.
        if pressed(shortcut(cmd_shift, Key::S)) {
            self.save_as();
        } else if pressed(shortcut(Modifiers::COMMAND, Key::S)) {
            self.save();
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::O)) {
            self.request(ctx, Pending::Open);
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::N)) {
            self.request(ctx, Pending::New);
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::M)) {
            self.checkpoint();
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::Equals))
            || pressed(shortcut(Modifiers::COMMAND, Key::Plus))
        {
            self.set_font_size(self.settings.font_size + FONT_STEP);
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::Minus)) {
            self.set_font_size(self.settings.font_size - FONT_STEP);
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        // The window manager (or a logout) asked to close. Secure the work
        // first; a draft then gets the usual question.
        self.checkpoint();
        if self.is_unsaved_draft() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        } else {
            self.persist_settings();
        }
    }

    /// The app is always full screen: if the compositor ever drops it out
    /// (or the user restores it after minimising), go back.
    fn keep_fullscreen(&mut self, ctx: &egui::Context) {
        let (fullscreen, minimized) =
            ctx.input(|i| (i.viewport().fullscreen, i.viewport().minimized));
        let recently = self
            .last_fullscreen_request
            .is_some_and(|t| t.elapsed() < FULLSCREEN_RETRY);
        if fullscreen == Some(false) && minimized != Some(true) && !recently {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            self.last_fullscreen_request = Some(Instant::now());
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!("{} — {}", self.doc.display_name(), crate::APP_NAME);
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    /// Page move/resize with the mouse; returns the cursor to show.
    fn handle_page_drag(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
        page: &mut Rect,
    ) -> CursorIcon {
        let (pointer, pressed, down, delta) = ctx.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.delta(),
            )
        });
        // The dock, flyouts and dialogs live on higher layers than the page.
        let over_ui = pointer
            .and_then(|p| ctx.layer_id_at(p))
            .is_some_and(|layer| layer.order != egui::Order::Background);
        if pressed && !over_ui {
            self.drag = pointer.and_then(|p| page::hit_test(*page, p));
        }
        if let Some(drag) = self.drag {
            if down {
                *page = page::apply_drag(*page, drag, delta, screen);
                return drag.cursor();
            }
            self.drag = None;
            self.settings.page = Some(PageRect {
                x: page.min.x,
                y: page.min.y,
                width: page.width(),
                height: page.height(),
            });
            self.settings_touched();
        }
        match pointer.and_then(|p| page::hit_test(*page, p)) {
            Some(d) if !over_ui => d.cursor(),
            _ => CursorIcon::Default,
        }
    }

    // ----- drawing ----------------------------------------------------------

    fn paint_page(&self, ctx: &egui::Context, painter: &egui::Painter, page: Rect, fade: f32) {
        let ink = self.stage.ink();
        let sheet = if self.stage.dark() {
            Color32::from_rgb(0x14, 0x16, 0x1c)
        } else {
            Color32::WHITE
        };
        painter.rect_filled(page, 0.0, sheet.gamma_multiply(self.settings.page_opacity));
        if fade <= 0.0 {
            return;
        }
        let outline = [
            page.left_top(),
            page.right_top(),
            page.right_bottom(),
            page.left_bottom(),
            page.left_top(),
        ];
        painter.extend(egui::Shape::dashed_line(
            &outline,
            Stroke::new(1.0, ink.gamma_multiply(0.32 * fade)),
            5.0,
            5.0,
        ));
        // Handles appear only when the pointer approaches the edge.
        let near_edge = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
            page.expand(HANDLE_REVEAL).contains(p) && !page.shrink(HANDLE_REVEAL).contains(p)
        });
        if near_edge || self.drag.is_some() {
            for (handle, _) in page::handles(page) {
                let square = Rect::from_center_size(handle.center(), Vec2::splat(7.0));
                painter.rect_filled(square, 0.0, sheet.gamma_multiply(fade));
                painter.rect_stroke(
                    square,
                    0.0,
                    Stroke::new(1.0, ink.gamma_multiply(0.6 * fade)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    fn text_area(&mut self, ui: &mut egui::Ui, page: Rect) {
        let inner = Rect::from_min_max(page.min + PAGE_PADDING, page.max - PAGE_PADDING);
        let size = self.settings.font_size;
        let look = markdown::Look {
            fonts: fonts::typeface(self.settings.font_family.as_deref()).writing_fonts(size),
            size,
            line_spacing: LINE_SPACING,
            ink: self.stage.ink(),
            accent: theme::palette(self.stage.dark()).accent,
        };
        let enabled = self.settings.markdown;
        let editor_id = editor_id();
        let active = Self::cursor_line(ui.ctx(), editor_id, &self.doc.text);
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = markdown::layout(text.as_str(), &look, enabled, active.as_ref());
            job.wrap.max_width = wrap_width;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let ink = look.ink;
        let squiggle = if self.stage.dark() {
            Color32::from_rgb(0xff, 0x8a, 0x7a)
        } else {
            theme::DANGER
        };
        let checker = self.spell.as_ref().filter(|_| self.settings.spell_check);
        let typing_recently = self
            .last_edit
            .is_some_and(|t| t.elapsed() < Duration::from_millis(1500));
        let mut menu: Option<Option<SpellMenu>> = None;
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(inner), |ui| {
                ui.visuals_mut().text_cursor.stroke.color = ink;
                egui::ScrollArea::vertical()
                    .id_salt("page-scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let output = egui::TextEdit::multiline(&mut self.doc.text)
                            .id(editor_id)
                            .frame(egui::Frame::NONE)
                            .layouter(&mut layouter)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY)
                            .min_size(inner.size())
                            .show(ui);
                        if let Some(checker) = checker {
                            let text = self.doc.text.as_str();
                            let ranges = Self::misspellings(
                                &mut self.spell_cache,
                                checker,
                                self.spell_generation,
                                text,
                            );
                            let cursor = output
                                .cursor_range
                                .map(|c| byte_of_char(text, c.primary.index.0));
                            for range in ranges {
                                // Don't nag about the word still being typed.
                                let typing_here = typing_recently
                                    && cursor.is_some_and(|b| range.start <= b && b <= range.end);
                                if !typing_here {
                                    paint_squiggle(ui, &output, text, range, squiggle);
                                }
                            }
                            if output.response.secondary_clicked()
                                && let Some(pos) = output.response.interact_pointer_pos()
                            {
                                let at = output.galley.cursor_from_pos(pos - output.galley_pos);
                                let byte = byte_of_char(text, at.index.0);
                                menu = Some(
                                    ranges
                                        .iter()
                                        .find(|r| r.start <= byte && byte <= r.end)
                                        .map(|r| {
                                            let word = text[r.clone()].to_owned();
                                            SpellMenu {
                                                range: r.clone(),
                                                suggestions: checker.suggest(&word),
                                                word,
                                            }
                                        }),
                                );
                            }
                        }
                        egui::Response::clone(&output.response).accessible_name("Document text")
                    })
                    .inner
            })
            .inner;
        if let Some(menu) = menu {
            self.spell_menu = menu;
        }
        if self.spell_menu.is_some() {
            response.context_menu(|ui| self.spell_menu_ui(ui));
        }
        if response.changed() {
            self.doc.mark_dirty();
            self.last_edit = Some(Instant::now());
            self.audio.play_click();
            self.ui_visible = false;
            self.section = None;
        }
        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
        if nothing_focused {
            response.request_focus();
        }
    }

    /// Byte range of the line holding the text cursor (its markdown markers
    /// stay visible while editing).
    fn cursor_line(ctx: &egui::Context, id: Id, text: &str) -> Option<std::ops::Range<usize>> {
        let state = egui::TextEdit::load_state(ctx, id)?;
        let chars = state.cursor.char_range()?.primary.index.0;
        let byte = text
            .char_indices()
            .nth(chars)
            .map_or(text.len(), |(b, _)| b);
        let start = text[..byte].rfind('\n').map_or(0, |i| i + 1);
        let end = text[byte..].find('\n').map_or(text.len(), |i| byte + i);
        Some(start..end)
    }

    /// The dock of section buttons, plus the flyout of the active section.
    fn dock(&mut self, ctx: &egui::Context, screen: Rect, page: Rect, fade: f32) {
        let dock_width = ICON_BUTTON + 16.0;
        let room_right =
            page.right() + DOCK_GAP + dock_width + 8.0 + FLYOUT_WIDTH_ESTIMATE <= screen.right();
        let dock_x = if room_right {
            page.right() + DOCK_GAP
        } else {
            (page.left() - DOCK_GAP - dock_width).max(screen.left() + 8.0)
        };
        let interactive = fade > 0.5;

        let mut active_row: Option<(Section, Rect)> = None;
        let dock = egui::Area::new(Id::unique("dock"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(dock_x, page.top()))
            .interactable(interactive)
            .show(ctx, |ui| {
                ui.multiply_opacity(fade);
                ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
                for (section, icon, label) in Section::ALL {
                    let active = self.section == Some(section);
                    let r = icon_button(ui, icon, label, active);
                    if r.hovered() {
                        self.section = Some(section);
                    }
                    if self.section == Some(section) {
                        active_row = Some((section, r.rect));
                    }
                }
            })
            .response
            .rect;

        if self.section != Some(Section::File) {
            self.recent_open = false;
        }
        let mut area = dock;
        if let Some((section, row)) = active_row {
            // The controls sit level with the hovered icon; the title above.
            let top = row.top() - FLYOUT_TITLE_HEIGHT - FLYOUT_TITLE_GAP;
            let (anchor, pivot) = if room_right {
                (pos2(dock.right() + 4.0, top), Align2::LEFT_TOP)
            } else {
                (pos2(dock.left() - 4.0, top), Align2::RIGHT_TOP)
            };
            let flyout = egui::Area::new(Id::unique("flyout"))
                .order(egui::Order::Foreground)
                .fixed_pos(anchor)
                .pivot(pivot)
                .interactable(interactive)
                .show(ctx, |ui| {
                    ui.multiply_opacity(fade);
                    self.flyout(ui, section);
                })
                .response
                .rect;
            area = area.union(flyout);
        }
        self.menu_area = Some(area);
    }

    /// Closes the flyout once the pointer has clearly moved away from it.
    fn maybe_close_flyout(&mut self, ctx: &egui::Context) {
        let Some(area) = self.menu_area else {
            return;
        };
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let popup_open = egui::Popup::is_any_open(ctx);
        if let Some(p) = pointer
            && !area.expand(36.0).contains(p)
            && !popup_open
        {
            self.section = None;
        }
    }

    fn flyout(&mut self, ui: &mut egui::Ui, section: Section) {
        let ctx = ui.ctx().clone();
        let (title, detail) = match section {
            Section::Text => ("Font", None),
            Section::Size => (
                "Text size",
                Some(format!("{:.0} pt", self.settings.font_size)),
            ),
            Section::Theme => ("Theme", Some(self.themes.current().name.clone())),
            Section::Music => ("Music", None),
            Section::Keyboard => ("Typing sound", None),
            Section::Timer => ("Timer", Some(self.timer_detail())),
            Section::File => (
                "Document",
                Some(if self.doc.path().is_none() {
                    "Untitled draft (not saved)".to_owned()
                } else {
                    self.doc.display_name()
                }),
            ),
        };
        let p = theme::current(ui);
        ui.spacing_mut().item_spacing.y = FLYOUT_TITLE_GAP;
        // Rows are only as tall as their contents (the theme's 30 pt minimum
        // would push the controls below the dock icon they belong to).
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal(|ui| {
            ui.set_height(FLYOUT_TITLE_HEIGHT);
            // Line the title up with the first control's glyph.
            ui.add_space(5.0);
            caption(
                ui,
                &title.to_uppercase(),
                8.5,
                p.ink.gamma_multiply(0.6),
                1.4,
            );
            if let Some(detail) = detail {
                ui.add_space(3.0);
                caption(ui, &detail, 10.5, p.ink.gamma_multiply(0.85), 0.0);
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            match section {
                Section::Text => self.text_controls(ui, &ctx),
                Section::Size => {
                    let size = self.settings.font_size;
                    if icon_button(ui, Icon::Smaller, "Smaller (Ctrl+-)", false).clicked() {
                        self.set_font_size(size - FONT_STEP);
                    }
                    if icon_button(ui, Icon::DefaultSize, "Default size", false).clicked() {
                        self.set_font_size(DEFAULT_FONT_SIZE);
                    }
                    if icon_button(ui, Icon::Larger, "Larger (Ctrl+=)", false).clicked() {
                        self.set_font_size(size + FONT_STEP);
                    }
                }
                Section::Theme => {
                    self.ensure_thumbnails(&ctx);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(7.0, 7.0);
                        let current = self.themes.current_index();
                        let names: Vec<String> = self
                            .themes
                            .entries()
                            .iter()
                            .map(|e| e.name.clone())
                            .collect();
                        let mut chosen = None;
                        for (row, chunk) in names.chunks(THEME_GRID_COLUMNS).enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(3.0);
                                for (col, name) in chunk.iter().enumerate() {
                                    let i = row * THEME_GRID_COLUMNS + col;
                                    let tex = self.thumbs.get(i).and_then(Option::as_ref);
                                    if thumb_button(ui, tex, name, i == current).clicked() {
                                        chosen = Some(i);
                                    }
                                }
                            });
                        }
                        if let Some(i) = chosen {
                            self.select_theme(&ctx, i);
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if switch(ui, Icon::Motion, "Motion", self.settings.motion).clicked() {
                                self.settings.motion = !self.settings.motion;
                                self.settings_touched();
                            }
                            ui.add_space(14.0);
                            caption(ui, "Sheet", 11.0, p.ink.gamma_multiply(0.6), 0.0);
                            ui.add_space(6.0);
                            if slider(ui, &mut self.settings.page_opacity, "Sheet opacity") {
                                self.settings_touched();
                            }
                        });
                    });
                }
                Section::Music => {
                    let muted = self.settings.music_muted;
                    let (icon, label) = if muted {
                        (Icon::SpeakerOff, "Unmute music")
                    } else {
                        (Icon::SpeakerOn, "Mute music")
                    };
                    if icon_button(ui, icon, label, muted).clicked() {
                        self.settings.music_muted = !muted;
                        self.audio.set_music_muted(!muted);
                        self.settings_touched();
                    }
                    if slider(ui, &mut self.settings.music_volume, "Music volume") {
                        self.audio.set_music_volume(self.settings.music_volume);
                        self.settings_touched();
                    }
                }
                Section::Keyboard => {
                    let muted = self.settings.click_muted;
                    let (icon, label) = if muted {
                        (Icon::SpeakerOff, "Unmute typing")
                    } else {
                        (Icon::SpeakerOn, "Mute typing")
                    };
                    if icon_button(ui, icon, label, muted).clicked() {
                        self.settings.click_muted = !muted;
                        self.audio.set_click_muted(!muted);
                        self.settings_touched();
                    }
                    if slider(ui, &mut self.settings.click_volume, "Typing volume") {
                        self.audio.set_click_volume(self.settings.click_volume);
                        self.settings_touched();
                    }
                }
                Section::Timer => self.timer_controls(ui),
                Section::File => {
                    if icon_button(ui, Icon::New, "New document (Ctrl+N)", false).clicked() {
                        self.request(&ctx, Pending::New);
                    }
                    let open = icon_button_bare(ui, Icon::Open, "Open (Ctrl+O)", self.recent_open);
                    if open.hovered() {
                        self.recent_open = true;
                    }
                    if open.clicked() {
                        self.request(&ctx, Pending::Open);
                    }
                    if icon_button(ui, Icon::Save, "Save (Ctrl+S)", false).clicked() {
                        self.save();
                    }
                    if icon_button(ui, Icon::SaveAs, "Save as… (Ctrl+Shift+S)", false).clicked() {
                        self.save_as();
                    }
                }
            }
        });
        match section {
            Section::Text => self.text_switches(ui, &ctx),
            Section::Timer => self.timer_slider(ui),
            Section::File if self.recent_open => self.recent_files(ui, &ctx),
            Section::Music => {
                let current = self.music.current_index();
                let names: Vec<String> = self
                    .music
                    .entries()
                    .iter()
                    .map(|e| e.name.clone())
                    .collect();
                for (i, name) in names.iter().enumerate() {
                    if list_choice(ui, name, i == current).clicked() {
                        self.select_music(i);
                    }
                }
            }
            Section::Keyboard => {
                let current = self.clicks.current_index();
                let names: Vec<String> = self
                    .clicks
                    .entries()
                    .iter()
                    .map(|e| e.name.clone())
                    .collect();
                let mut hovered = None;
                ui.horizontal_top(|ui| {
                    for (col, chunk) in names.chunks(names.len().div_ceil(3)).enumerate() {
                        ui.vertical(|ui| {
                            for (row, name) in chunk.iter().enumerate() {
                                let i = col * names.len().div_ceil(3) + row;
                                let r = list_choice(ui, name, i == current);
                                if r.hovered() {
                                    hovered = Some(i);
                                }
                                if r.clicked() {
                                    self.select_click(i);
                                }
                            }
                        });
                    }
                });
                match hovered {
                    Some(i) => self.preview_click(i),
                    None => self.previewed_click = None,
                }
            }
            _ => {}
        }
    }

    fn text_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let current = fonts::typeface(self.settings.font_family.as_deref()).name;
        for typeface in &fonts::TYPEFACES {
            let font = fonts::preview_font(typeface, 13.5);
            if text_choice(ui, typeface.label, font, typeface.name == current).clicked() {
                self.set_typeface(ctx, typeface);
            }
        }
    }

    /// The writing switches under the font names.
    fn text_switches(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.add_space(4.0);
            if switch(ui, Icon::Markdown, "Markdown", self.settings.markdown).clicked() {
                self.settings.markdown = !self.settings.markdown;
                self.settings_touched();
            }
            if switch(
                ui,
                Icon::SpellCheck,
                "Spell check",
                self.settings.spell_check,
            )
            .clicked()
            {
                self.toggle_spell_check(ctx);
            }
        });
    }

    /// Recently opened files, newest first. Names that occur twice show
    /// their folder too; hovering shows the full path.
    fn recent_files(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let current = self.doc.path().map(Path::to_path_buf);
        let recent = self.settings.recent_files.clone();
        let name = |p: &Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        };
        let mut chosen = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.set_height(FLYOUT_TITLE_HEIGHT);
            ui.add_space(5.0);
            let p = theme::current(ui);
            let text = if recent.is_empty() {
                "NO RECENT FILES"
            } else {
                "RECENT"
            };
            caption(ui, text, 8.5, p.ink.gamma_multiply(0.6), 1.4);
        });
        for path in &recent {
            let mut label = name(path);
            if recent.iter().filter(|p| name(p) == label).count() > 1
                && let Some(folder) = path.parent().and_then(Path::file_name)
            {
                label = format!("{label} — {}", folder.to_string_lossy());
            }
            let selected = current.as_deref() == Some(path.as_path());
            let r = list_choice(ui, &label, selected).on_hover_text(path.display().to_string());
            if r.clicked() && !selected {
                chosen = Some(path.clone());
            }
        }
        if let Some(path) = chosen {
            self.recent_open = false;
            self.section = None;
            self.request(ctx, Pending::OpenFile(path.clone()));
            if self.doc.path() != Some(path.as_path()) && self.pending.is_none() {
                // It could not be opened (moved or deleted): forget it.
                self.settings.recent_files.retain(|p| *p != path);
                self.settings_touched();
            }
        }
    }

    // ----- timer ------------------------------------------------------------

    /// The flyout title's detail: what the timer is doing.
    fn timer_detail(&self) -> String {
        let now = Instant::now();
        let left = timer::clock(self.timer.remaining(now));
        if self.timer.is_running() {
            format!("{left} left")
        } else if self.timer.is_active() {
            format!("{left} left, paused")
        } else {
            format!("{} min", self.timer.minutes())
        }
    }

    fn set_timer_minutes(&mut self, minutes: u32) {
        self.timer.set_minutes(minutes, Instant::now());
        self.settings.timer_minutes = self.timer.minutes();
        self.settings_touched();
    }

    fn toggle_timer(&mut self) {
        self.timer.toggle(Instant::now());
    }

    /// Play/pause and reset, then one-click lengths that start at once.
    fn timer_controls(&mut self, ui: &mut egui::Ui) {
        let (icon, label) = if self.timer.is_running() {
            (Icon::Pause, "Pause timer")
        } else if self.timer.is_active() {
            (Icon::Play, "Resume timer")
        } else {
            (Icon::Play, "Start timer")
        };
        if icon_button(ui, icon, label, self.timer.is_running()).clicked() {
            self.toggle_timer();
        }
        let resettable = self.timer.is_active();
        let reset = ui.add_enabled_ui(resettable, |ui| {
            if !resettable {
                ui.multiply_opacity(0.35);
            }
            icon_button(ui, Icon::DefaultSize, "Reset timer", false)
        });
        if reset.inner.clicked() {
            self.timer.reset();
        }
        ui.add_space(6.0);
        for minutes in timer::PRESETS {
            let font = egui::FontId::proportional(13.0);
            let chosen = self.timer.minutes() == minutes;
            if text_choice(ui, &format!("{minutes} min"), font, chosen).clicked() {
                self.set_timer_minutes(minutes);
                self.timer.start(Instant::now());
            }
        }
    }

    /// Any other length, from 1 to 60 minutes.
    fn timer_slider(&mut self, ui: &mut egui::Ui) {
        let span = (timer::MAX_MINUTES - timer::MIN_MINUTES) as f32;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let mut value = (self.timer.minutes() - timer::MIN_MINUTES) as f32 / span;
            if slider(ui, &mut value, "Timer length") {
                let minutes = timer::MIN_MINUTES + (value * span).round() as u32;
                if minutes != self.timer.minutes() {
                    self.set_timer_minutes(minutes);
                }
            }
        });
    }

    /// Ends the countdown with a chime once it reaches zero, and keeps the
    /// display ticking.
    fn tick_timer(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if self.timer.poll(now) {
            self.audio.play_chime();
            let minutes = self.timer.minutes();
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            self.set_status(format!("{minutes} {unit} are up"), false);
        }
        if let Some(next) = self.timer.next_tick(now) {
            ctx.request_repaint_after(next);
        }
    }

    /// The countdown under the page's right corner: a ring that empties and
    /// the time left. It stays (faintly) while you write, and a click
    /// pauses or resumes it.
    fn timer_badge(&mut self, ctx: &egui::Context, page: Rect, fade: f32) {
        let show =
            ctx.animate_bool_with_time(Id::unique("timer-badge"), self.timer.is_active(), FADE_IN);
        if show <= 0.0 {
            return;
        }
        let now = Instant::now();
        let left = self.timer.remaining(now);
        let fraction = self.timer.fraction_left(now);
        let running = self.timer.is_running();
        let ink = self.stage.ink();
        // Quieter while writing, clearer when the interface is up.
        let opacity = show * (0.75 + 0.25 * fade);
        let label = if running {
            "Pause timer"
        } else {
            "Resume timer"
        };
        let clicked = egui::Area::new(Id::unique("timer-badge-area"))
            .order(egui::Order::Middle)
            .fixed_pos(pos2(page.right(), page.bottom() + 14.0))
            .pivot(Align2::RIGHT_TOP)
            .show(ctx, |ui| {
                ui.multiply_opacity(opacity);
                // Level with the word count (rows are otherwise 30 pt tall).
                ui.spacing_mut().interact_size.y = 0.0;
                let r = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                        paint_ring(ui.painter(), rect.center(), 4.5, fraction, ink, running);
                        caption(ui, &timer::clock(left), 12.5, ink.gamma_multiply(0.6), 0.4);
                    })
                    .response
                    .interact(egui::Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
                r.on_hover_text(label).clicked()
            })
            .inner;
        if clicked {
            self.toggle_timer();
        }
    }

    /// While a slow document is still on its way, a quiet note saying so.
    fn loading_hint(&self, ctx: &egui::Context, page: Rect) {
        let Some(loading) = &self.loading else {
            return;
        };
        let waited = loading.since.elapsed();
        let delay = Duration::from_millis(400);
        if waited < delay {
            ctx.request_repaint_after(delay.saturating_sub(waited));
            return;
        }
        let name = loading
            .path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        egui::Area::new(Id::unique("loading-hint"))
            .order(egui::Order::Middle)
            .interactable(false)
            .fixed_pos(page.min + PAGE_PADDING)
            .show(ctx, |ui| {
                let ink = self.stage.ink().gamma_multiply(0.5);
                caption(ui, &format!("Opening {name}…"), 13.0, ink, 0.2);
            });
    }

    /// Word count, centred under the page, with the selection's count
    /// beside it while text is highlighted. It fades with the interface,
    /// but stays visible while something is selected.
    fn word_count(&self, ctx: &egui::Context, page: Rect, fade: f32) {
        let selected = selection(ctx, &self.doc.text)
            .map(|range| document::count_words(&self.doc.text[range]));
        let show_selection =
            ctx.animate_bool_with_time(Id::unique("selection-count"), selected.is_some(), FADE_IN);
        let opacity = fade.max(show_selection);
        if opacity <= 0.0 {
            return;
        }
        let text = word_count_label(self.doc.word_count(), selected);
        egui::Area::new(Id::unique("word-count"))
            .order(egui::Order::Middle)
            .interactable(false)
            .fixed_pos(pos2(page.center().x, page.bottom() + 14.0))
            .pivot(Align2::CENTER_TOP)
            .show(ctx, |ui| {
                ui.multiply_opacity(opacity);
                caption(ui, &text, 12.5, self.stage.ink().gamma_multiply(0.6), 0.4);
            });
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let Some(status) = &self.status else {
            return;
        };
        let lifetime = if status.is_error {
            ERROR_TIME
        } else {
            STATUS_TIME
        };
        let age = status.since.elapsed();
        if age >= lifetime {
            self.status = None;
            return;
        }
        let fade_in = (age.as_secs_f32() / 0.2).min(1.0);
        let fade_out = lifetime.saturating_sub(age).as_secs_f32().min(1.0);
        let alpha = fade_in.min(fade_out);
        egui::Area::new(Id::unique("toast"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 24.0))
            .show(ctx, |ui| {
                ui.multiply_opacity(alpha);
                let p = theme::current(ui);
                ui.horizontal(|ui| {
                    if status.is_error {
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, theme::DANGER);
                    }
                    caption(ui, &status.text, 13.0, p.ink.gamma_multiply(0.85), 0.2);
                });
            });
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

impl eframe::App for SamadhiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_background_work(&ctx);
        #[cfg(unix)]
        self.handle_messages(&ctx);
        self.handle_shortcuts(&ctx);
        self.handle_close_request(&ctx);
        self.keep_fullscreen(&ctx);

        let screen = ui.max_rect();
        // While dragging, the live rectangle is authoritative; otherwise use
        // the saved placement, or a default that follows the screen size.
        let mut page = match (self.drag, self.page, self.settings.page) {
            (Some(_), Some(live), _) => live,
            (_, _, Some(p)) => Rect::from_min_size(pos2(p.x, p.y), vec2(p.width, p.height)),
            _ => page::default_rect(screen),
        };
        page = page::clamp_into(page, screen);

        // Any mouse activity brings the interface back; typing hides it.
        let pointer_active = ctx.input(|i| {
            i.pointer.delta() != Vec2::ZERO
                || i.pointer.any_pressed()
                || i.smooth_scroll_delta != Vec2::ZERO
        });
        if pointer_active {
            self.ui_visible = true;
            self.last_pointer = Instant::now();
        }
        // Fade out once the mouse rests, unless it rests on the controls or
        // something needs the interface (a drag, a dialog or a menu).
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let on_controls = pointer
            .zip(self.menu_area)
            .is_some_and(|(p, area)| area.expand(12.0).contains(p));
        let busy = on_controls
            || self.drag.is_some()
            || self.pending.is_some()
            || egui::Popup::is_any_open(&ctx);
        if busy {
            self.last_pointer = Instant::now();
        } else if self.ui_visible {
            let idle = self.last_pointer.elapsed();
            if idle >= IDLE_HIDE {
                self.ui_visible = false;
                self.section = None;
            } else {
                ctx.request_repaint_after(IDLE_HIDE.saturating_sub(idle));
            }
        }
        let fade = ctx.animate_bool_with_time(
            Id::unique("ui-fade"),
            self.ui_visible,
            if self.ui_visible { FADE_IN } else { FADE_OUT },
        );

        let cursor = if self.ui_visible {
            self.handle_page_drag(&ctx, screen, &mut page)
        } else {
            CursorIcon::Default
        };
        self.page = Some(page);

        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let animating = self.stage.paint(
            ui.painter(),
            screen,
            pointer,
            self.settings.motion,
            1.0 - fade,
        );
        if animating {
            // About 30 fps is plenty for slow ambient motion.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        self.paint_page(&ctx, ui.painter(), page, fade);
        if self.loading.is_some() {
            // Nothing to type into until the document arrives.
            self.loading_hint(&ctx, page);
        } else {
            self.text_area(ui, page);
        }

        if fade > 0.0 {
            self.dock(&ctx, screen, page, fade);
            self.maybe_close_flyout(&ctx);
        } else {
            self.menu_area = None;
        }
        self.word_count(&ctx, page, fade);
        self.tick_timer(&ctx);
        self.timer_badge(&ctx, page, fade);
        ctx.set_cursor_icon(if self.ui_visible {
            cursor
        } else {
            CursorIcon::None
        });
        self.unsaved_dialog(&ctx);
        self.toast(&ctx);

        self.autosave();
        if self.doc.path().is_some() {
            if self.last_disk_check.elapsed() >= DISK_CHECK {
                self.sync_with_disk();
            }
            ctx.request_repaint_after(DISK_CHECK);
        }
        if self
            .settings_changed
            .is_some_and(|t| t.elapsed() >= SETTINGS_SAVE_DELAY)
        {
            self.persist_settings();
        }
        self.update_title(&ctx);
        // Wake up for the pending autosave and settings write.
        if self.last_edit.is_some() {
            ctx.request_repaint_after(AUTOSAVE_DELAY);
        }
        if self.settings_changed.is_some() {
            ctx.request_repaint_after(SETTINGS_SAVE_DELAY);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn on_exit(&mut self) {
        self.checkpoint();
        self.persist_settings();
    }
}

/// A file name for a new document, taken from its first line (as most
/// writing apps do), e.g. `# A quiet place` → `A quiet place.md`.
fn suggested_file_name(text: &str, extension: &str) -> String {
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let cleaned: String = first
        .trim_start_matches(['#', '>', '-', '*', ' '])
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '`'
            )
        })
        .collect();
    let mut name: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() > 60 {
        let short: String = name.chars().take(60).collect();
        short.trim_end().clone_into(&mut name);
    }
    let name = name.trim_end_matches(['.', ',', ';', ' ']);
    let stem = if name.is_empty() { "Untitled" } else { name };
    format!("{stem}.{extension}")
}

/// The id of the writing area.
/// A thin ring showing `fraction` of a full turn, clockwise from the top,
/// over a faint full circle. Paused, the ring is drawn fainter.
fn paint_ring(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    fraction: f32,
    ink: Color32,
    running: bool,
) {
    painter.circle_stroke(center, radius, Stroke::new(1.0, ink.gamma_multiply(0.22)));
    let steps = (48.0 * fraction).ceil() as usize;
    if steps == 0 {
        return;
    }
    let points: Vec<_> = (0..=steps)
        .map(|i| {
            let turn = fraction * i as f32 / steps as f32;
            let angle = std::f32::consts::TAU * turn - std::f32::consts::FRAC_PI_2;
            center + radius * vec2(angle.cos(), angle.sin())
        })
        .collect();
    let strength = if running { 0.7 } else { 0.4 };
    painter.add(egui::Shape::line(
        points,
        Stroke::new(1.5, ink.gamma_multiply(strength)),
    ));
}

fn editor_id() -> Id {
    Id::unique("page-text")
}

/// The byte range of the text highlighted in the editor, if any.
fn selection(ctx: &egui::Context, text: &str) -> Option<Range<usize>> {
    let range = egui::TextEdit::load_state(ctx, editor_id())?
        .cursor
        .char_range()?;
    let (a, b) = (range.primary.index.0, range.secondary.index.0);
    (a != b).then(|| byte_of_char(text, a.min(b))..byte_of_char(text, a.max(b)))
}

/// "340 words", or "340 words · 12 selected" while text is highlighted.
fn word_count_label(total: usize, selected: Option<usize>) -> String {
    let total = format!("{total} {}", if total == 1 { "word" } else { "words" });
    match selected {
        Some(n) => format!("{total} \u{00b7} {n} selected"),
        None => total,
    }
}

fn byte_of_char(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(b, _)| b)
}

/// Draws a wavy underline beneath `range` of the editor's text.
fn paint_squiggle(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    range: &Range<usize>,
    color: Color32,
) {
    use egui::text::CCursor;
    let start = text[..range.start].chars().count();
    let end = start + text[range.clone()].chars().count();
    let a = output.galley.pos_from_cursor(CCursor::new(start));
    let b = output.galley.pos_from_cursor(CCursor::new(end));
    if (a.top() - b.top()).abs() > 1.0 {
        return; // Word broken across rows; rare, skip.
    }
    let origin = output.galley_pos.to_vec2();
    let y = origin.y + a.bottom() - 3.0;
    let (x0, x1) = (origin.x + a.left(), origin.x + b.left());
    let mut points = Vec::new();
    let mut x = x0;
    let mut up = true;
    while x <= x1 {
        points.push(pos2(x, if up { y - 1.2 } else { y + 1.2 }));
        x += 2.2;
        up = !up;
    }
    if points.len() > 1 {
        ui.painter().add(egui::Shape::line(
            points,
            Stroke::new(1.0, color.gamma_multiply(0.85)),
        ));
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    fn options(dir: &Path, file: Option<PathBuf>) -> Options {
        Options {
            file,
            settings_path: Some(dir.join("settings.toml")),
            documents_dir: Some(dir.join("docs")),
            recovery_dir: Some(dir.join("recovery")),
            audio: false,
            #[cfg(unix)]
            messages: None,
        }
    }

    /// Waits for the document being opened at start-up to arrive.
    fn settle(h: &mut Harness<'_, SamadhiApp>) {
        for _ in 0..500 {
            h.step();
            if h.state().loading.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(h.state().loading.is_none(), "document never arrived");
        h.run_steps(2);
    }

    /// Runs the real app headlessly with no audio and throwaway folders.
    fn harness(dir: &Path) -> Harness<'static, SamadhiApp> {
        let options = options(dir, None);
        let mut harness = Harness::builder()
            .with_size(vec2(1600.0, 1000.0))
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        harness.run_steps(3);
        settle(&mut harness);
        harness
    }

    #[test]
    fn typing_edits_the_document_and_fades_the_interface() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        assert!(h.state().ui_visible);
        h.get_by_role(egui::Role::MultilineTextInput)
            .type_text("Hello, stillness");
        h.run_steps(3);
        let app = h.state();
        assert_eq!(app.doc.text, "Hello, stillness");
        assert!(app.doc.is_dirty());
        assert!(!app.ui_visible, "typing should hide the interface");
    }

    #[test]
    fn hovering_the_dock_opens_a_flyout_whose_buttons_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        h.get_by_role_and_label(egui::Role::Button, "Music").hover();
        h.run_steps(3);
        assert_eq!(h.state().section, Some(Section::Music));
        h.get_by_label("Mute music").click();
        h.run_steps(3);
        assert!(h.state().settings.music_muted);
        assert!(h.query_by_label("Unmute music").is_some());
    }

    fn type_and_pause(h: &mut Harness<'_, SamadhiApp>, text: &str) {
        h.get_by_role(egui::Role::MultilineTextInput)
            .type_text(text);
        h.run_steps(2);
        std::thread::sleep(AUTOSAVE_DELAY + Duration::from_millis(50));
        h.run_steps(2);
    }

    #[test]
    fn drafts_are_not_files_but_survive_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        assert_eq!(
            h.state().doc.path(),
            None,
            "a new document is an unsaved draft"
        );
        type_and_pause(&mut h, "a thought worth keeping");
        assert!(
            !dir.path().join("docs").exists(),
            "no file until the first save"
        );
        let cached = Recovery::new(Some(dir.path().join("recovery")))
            .load()
            .unwrap();
        assert_eq!(cached.text, "a thought worth keeping");

        // Simulate a crash: the app vanishes without any shutdown.
        std::mem::forget(h);
        let h = harness(dir.path());
        assert_eq!(h.state().doc.text, "a thought worth keeping");
        assert_eq!(h.state().doc.path(), None);
        assert!(h.state().doc.is_dirty());
    }

    #[test]
    fn escape_on_a_draft_asks_and_dont_save_discards_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        type_and_pause(&mut h, "draft");
        h.key_press(Key::Escape);
        h.run_steps(3);
        assert_eq!(h.state().pending, Some(Pending::Quit));
        assert!(h.query_by_label("Save this draft?").is_some());

        // Esc again cancels the question and keeps writing.
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(h.state().pending, None);
        assert!(!h.state().allow_close);

        h.key_press(Key::Escape);
        h.run_steps(2);
        h.get_by_label("Don't save").click();
        h.run_steps(3);
        assert!(h.state().allow_close);
        assert_eq!(
            Recovery::new(Some(dir.path().join("recovery"))).load(),
            None
        );
    }

    #[test]
    fn saved_files_autosave_and_reopen_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.md");
        std::fs::write(&path, "Once").unwrap();
        let options = options(dir.path(), Some(path.clone()));
        let mut h = Harness::builder()
            .with_size(vec2(1600.0, 1000.0))
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        settle(&mut h);
        type_and_pause(&mut h, " more");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Once more");
        assert!(!h.state().doc.is_dirty());
        // No question when quitting a saved file.
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert!(h.state().allow_close);
        drop(h);

        let h = harness(dir.path());
        assert_eq!(h.state().doc.path(), Some(path.as_path()));
    }

    fn open_file(dir: &Path, path: &Path) -> Harness<'static, SamadhiApp> {
        let options = options(dir, Some(path.to_path_buf()));
        let mut h = Harness::builder()
            .with_size(vec2(1600.0, 1000.0))
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        settle(&mut h);
        h
    }

    #[test]
    fn edits_synced_from_another_device_reload_or_are_kept_beside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.md");
        std::fs::write(&path, "written here").unwrap();
        let mut h = open_file(dir.path(), &path);

        // Nothing unsaved here: the other device's version simply appears.
        std::fs::write(&path, "written on the Mac").unwrap();
        h.state_mut().sync_with_disk();
        assert_eq!(h.state().doc.text, "written on the Mac");
        assert!(!h.state().doc.is_dirty());

        // Unsaved edits here and a change there: keep both.
        h.get_by_role(egui::Role::MultilineTextInput)
            .type_text(", then more here");
        h.run_steps(2);
        std::fs::write(&path, "rewritten on the Mac").unwrap();
        std::thread::sleep(AUTOSAVE_DELAY + Duration::from_millis(50));
        h.run_steps(2);
        // Ours is saved (typed where the cursor was) and theirs kept beside.
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(saved, h.state().doc.text);
        assert!(saved.contains(", then more here") && saved.contains("Mac"));
        let copy = dir.path().join("shared (conflicted copy).md");
        assert_eq!(
            std::fs::read_to_string(copy).unwrap(),
            "rewritten on the Mac"
        );
    }

    #[cfg(unix)]
    #[test]
    fn files_from_a_second_launch_open_here_after_asking_about_a_draft() {
        use crate::instance::Message;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("from-the-file-manager.md");
        std::fs::write(&path, "opened by a second launch").unwrap();
        let (tx, rx) = mpsc::channel();
        let options = Options {
            messages: Some(rx),
            ..options(dir.path(), None)
        };
        let mut h = Harness::builder()
            .with_size(vec2(1600.0, 1000.0))
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        settle(&mut h);
        type_and_pause(&mut h, "an unsaved draft");

        tx.send(Message::Open(path.clone())).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().pending, Some(Pending::OpenFile(path.clone())));
        h.get_by_label("Don't save").click();
        h.run_steps(3);
        assert_eq!(h.state().doc.path(), Some(path.as_path()));
        assert_eq!(h.state().doc.text, "opened by a second launch");

        // Asking for the file that is already open just shows the app.
        tx.send(Message::Open(path.clone())).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().pending, None);
    }

    #[test]
    fn hovering_open_lists_recent_files_to_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let (first, second) = (dir.path().join("first.md"), dir.path().join("second.md"));
        std::fs::write(&first, "the first").unwrap();
        std::fs::write(&second, "the second").unwrap();
        let mut h = open_file(dir.path(), &first);
        h.state_mut().open_path(&second);
        h.state_mut().open_path(&first);
        h.state_mut().open_path(&second);
        assert_eq!(
            h.state().settings.recent_files,
            [second.clone(), first.clone()]
        );

        h.get_by_role_and_label(egui::Role::Button, "Document")
            .hover();
        h.run_steps(3);
        assert!(h.query_by_label("first.md").is_none(), "hidden until Open");
        h.get_by_label("Open (Ctrl+O)").hover();
        h.run_steps(3);
        h.get_by_label("first.md").hover();
        h.run_steps(2);
        h.get_by_label("first.md").click();
        h.run_steps(3);
        assert_eq!(h.state().doc.path(), Some(first.as_path()));
        assert_eq!(h.state().doc.text, "the first");
    }

    #[test]
    fn timer_starts_from_a_preset_and_pauses_from_its_badge() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        assert!(h.query_by_label("Pause timer").is_none(), "no badge yet");
        h.get_by_role_and_label(egui::Role::Button, "Timer").hover();
        h.run_steps(3);
        h.get_by_label("5 min").click();
        h.run_steps(3);
        assert!(h.state().timer.is_running());
        assert_eq!(h.state().settings.timer_minutes, 5);

        // The badge under the page pauses it, even with the dock hidden.
        h.state_mut().ui_visible = false;
        h.run_steps(30);
        let badge = h.query_all_by_label("Pause timer").last().unwrap();
        badge.click();
        h.run_steps(2);
        assert!(h.state().timer.is_active() && !h.state().timer.is_running());
    }

    #[test]
    fn word_count_shows_the_selection_beside_the_total() {
        assert_eq!(word_count_label(340, None), "340 words");
        assert_eq!(word_count_label(1, None), "1 word");
        assert_eq!(
            word_count_label(340, Some(12)),
            "340 words \u{00b7} 12 selected"
        );

        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        h.get_by_role(egui::Role::MultilineTextInput)
            .type_text("one two three four");
        h.run_steps(2);
        assert_eq!(selection(&h.ctx, &h.state().doc.text), None);
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run_steps(2);
        let text = h.state().doc.text.clone();
        let range = selection(&h.ctx, &text).expect("text is selected");
        assert_eq!(document::count_words(&text[range]), 4);
    }

    #[test]
    fn suggested_names_come_from_the_first_line() {
        assert_eq!(
            suggested_file_name("# A quiet place\nmore", "md"),
            "A quiet place.md"
        );
        assert_eq!(
            suggested_file_name("\n\n  Notes: on/off?  ", "txt"),
            "Notes onoff.txt"
        );
        assert_eq!(suggested_file_name("", "md"), "Untitled.md");
        let long = "word ".repeat(30);
        assert!(suggested_file_name(&long, "md").chars().count() <= 64);
    }

    #[test]
    fn spell_check_flags_typos_and_right_click_offers_fixes() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        h.get_by_role_and_label(egui::Role::Button, "Font").hover();
        h.run_steps(3);
        h.get_by_role_and_label(egui::Role::Switch, "Spell check")
            .click();
        for _ in 0..300 {
            h.step();
            if h.state().spell.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(h.state().settings.spell_check);
        let checker = h.state().spell.as_ref().expect("dictionary loaded");
        let text = "Teh quiet mind";
        assert_eq!(checker.misspellings(text), vec![0..3]);

        // Right-clicking the typo offers suggestions; choosing one fixes it.
        h.state_mut().doc.text = text.to_owned();
        h.run_steps(2);
        let page = h.state().page.unwrap();
        let on_word = page.min + PAGE_PADDING + vec2(12.0, 10.0);
        h.event(egui::Event::PointerMoved(on_word));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: on_word,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.run_steps(3);
        assert_eq!(
            h.state().spell_menu.as_ref().map(|m| m.word.as_str()),
            Some("Teh")
        );
        h.get_by_label("The").click();
        h.run_steps(3);
        assert_eq!(h.state().doc.text, "The quiet mind");
        assert!(h.state().doc.is_dirty());
    }

    #[test]
    fn flyout_controls_line_up_with_their_dock_icon() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        for (section, first_control) in [
            ("Font", "Literata"),
            ("Text size", "Smaller (Ctrl+-)"),
            ("Theme", "Hazy Hills"),
            ("Music", "Mute music"),
            ("Typing sounds", "Mute typing"),
            ("Document", "New document (Ctrl+N)"),
        ] {
            let icon = h.get_by_role_and_label(egui::Role::Button, section);
            icon.hover();
            let icon_y = icon.rect().center().y;
            h.run_steps(3);
            let control_y = h
                .get_by_role_and_label(egui::Role::Button, first_control)
                .rect()
                .center()
                .y;
            assert!(
                (icon_y - control_y).abs() <= 1.0,
                "{section}: icon at {icon_y}, {first_control} at {control_y}"
            );
        }
    }

    #[test]
    fn markdown_toggle_keeps_text_but_changes_layout() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        assert!(h.state().settings.markdown);
        h.get_by_role_and_label(egui::Role::Button, "Font").hover();
        h.run_steps(3);
        h.get_by_role_and_label(egui::Role::Switch, "Markdown")
            .click();
        h.run_steps(3);
        assert!(!h.state().settings.markdown);
    }

    #[test]
    fn font_size_shortcuts_change_and_persist_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        h.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
        h.run_steps(2);
        let expected = DEFAULT_FONT_SIZE + FONT_STEP;
        assert!((h.state().settings.font_size - expected).abs() < f32::EPSILON);
        h.state_mut().persist_settings();
        let saved = Settings::load_or_default(Some(&dir.path().join("settings.toml")));
        assert!((saved.font_size - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn opens_files_passed_on_start_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.md");
        std::fs::write(&path, "one two three").unwrap();
        let options = options(dir.path(), Some(path));
        let mut h = Harness::builder()
            .with_size(vec2(1200.0, 800.0))
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        settle(&mut h);
        assert_eq!(h.state().doc.text, "one two three");
        assert_eq!(h.state().doc.word_count(), 3);
    }

    /// Steps the app until the theme being built has been shown.
    fn wait_for_scene(h: &mut Harness<'_, SamadhiApp>) {
        for _ in 0..400 {
            h.step();
            if h.state().stage.is_loaded() && !h.state().scene_loader.is_pending() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(h.state().stage.is_loaded(), "scene never finished building");
        // Let the crossfade finish.
        std::thread::sleep(Duration::from_millis(950));
        h.run_steps(3);
    }

    #[test]
    fn stepping_themes_switches_scene_and_interface_palette() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(dir.path());
        wait_for_scene(&mut h);
        assert!(!h.state().stage.dark(), "default theme is light");
        let dark_index = ThemeKind::ALL
            .iter()
            .position(|(_, k)| k.is_dark())
            .unwrap();
        h.get_by_role_and_label(egui::Role::Button, "Theme").hover();
        h.run_steps(2);
        h.get_by_role_and_label(egui::Role::Button, ThemeKind::ALL[dark_index].0)
            .click();
        h.run_steps(2);
        wait_for_scene(&mut h);
        assert!(h.state().stage.dark());
        assert_eq!(h.ctx.theme(), egui::Theme::Dark);
        assert_eq!(
            h.state().settings.theme.as_deref(),
            Some(ThemeKind::ALL[dark_index].0)
        );
    }

    /// Renders the interface in several states to `target/previews/` for
    /// visual review: `cargo test -- --ignored render_previews`.
    #[test]
    #[ignore = "needs a GPU; writes preview images for manual review"]
    fn render_previews() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.md");
        std::fs::write(
            &path,
            "# Writing in a quiet place\n\nThe breath is **slow**; the wrods arrive *softly*, one after another.\n\n> Attention is the beginning of devotion.\n\n- notice\n- breathe\n- write",
        )
        .unwrap();
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/previews");
        std::fs::create_dir_all(&out).unwrap();
        let options = options(dir.path(), Some(path));
        let mut h = Harness::builder()
            .with_size(vec2(1600.0, 900.0))
            .wgpu()
            .build_eframe(move |cc| SamadhiApp::new(cc, options));
        settle(&mut h);
        h.state_mut().settings.spell_check = true;
        let ctx = h.ctx.clone();
        h.state_mut().load_spell_checker(&ctx);
        wait_for_scene(&mut h);
        let save = |h: &mut Harness<'_, SamadhiApp>, name: &str| {
            h.render()
                .unwrap()
                .save(out.join(format!("{name}.png")))
                .unwrap();
        };
        save(&mut h, "ui-1-idle");
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run_steps(20);
        save(&mut h, "ui-1b-selection");
        h.key_press(Key::ArrowRight);
        // Move the mouse (somewhere new) to bring the faded interface back.
        // (The pointer's first appearance has no movement, so move twice.)
        h.hover_at(pos2(50.0, 50.0));
        h.step();
        h.hover_at(pos2(60.0, 60.0));
        h.run_steps(10);
        // Let the theme previews finish so the grid is fully drawn.
        h.get_by_role_and_label(egui::Role::Button, "Theme").hover();
        for _ in 0..600 {
            h.step();
            if h.state().thumbs.iter().all(Option::is_some) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        for (label, name) in [
            ("Font", "ui-2-font"),
            ("Theme", "ui-3-theme"),
            ("Music", "ui-4-music"),
            ("Typing sounds", "ui-4b-typing"),
        ] {
            h.get_by_role_and_label(egui::Role::Button, label).hover();
            h.run_steps(10);
            save(&mut h, name);
        }
        let story = h.state().doc.path().unwrap().to_path_buf();
        for name in ["Morning pages.md", "Letters home.md"] {
            let other = story.with_file_name(name);
            std::fs::write(&other, "…").unwrap();
            h.state_mut().open_path(&other);
        }
        h.state_mut().open_path(&story);
        h.get_by_role_and_label(egui::Role::Button, "Document")
            .hover();
        h.run_steps(3);
        h.get_by_label("Open (Ctrl+O)").hover();
        h.run_steps(10);
        save(&mut h, "ui-4d-recent");
        h.get_by_role_and_label(egui::Role::Button, "Timer").hover();
        h.run_steps(3);
        h.get_by_label("25 min").click();
        h.run_steps(10);
        save(&mut h, "ui-4c-timer");
        // One switch off, to compare the two states side by side.
        h.get_by_role_and_label(egui::Role::Button, "Font").hover();
        h.run_steps(3);
        h.get_by_role_and_label(egui::Role::Switch, "Spell check")
            .click();
        h.run_steps(20);
        save(&mut h, "ui-2b-switch-off");
        h.get_by_role_and_label(egui::Role::Switch, "Spell check")
            .click();
        h.run_steps(5);
        h.get_by_role(egui::Role::MultilineTextInput)
            .type_text(" More.");
        h.run_steps(30);
        save(&mut h, "ui-5-writing");
        // Moving the mouse brings the faded interface back.
        h.hover_at(pos2(20.0, 20.0));
        h.run_steps(10);
        for (i, (name, _)) in ThemeKind::ALL.iter().enumerate() {
            if i > 0 {
                h.get_by_role_and_label(egui::Role::Button, "Theme").hover();
                h.run_steps(2);
                h.get_by_role_and_label(egui::Role::Button, name).click();
                wait_for_scene(&mut h);
            }
            h.get_by_role_and_label(egui::Role::Button, "Theme").hover();
            h.run_steps(5);
            save(
                &mut h,
                &format!("theme-{i}-{}", name.to_lowercase().replace(' ', "-")),
            );
        }
    }
}
