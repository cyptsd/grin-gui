mod element;
mod style;
mod update;

use crate::cli::Opts;
use crate::localization::{localized_string, LANG};
use ajour_core::{config::Config, theme::Theme};
use uuid::Uuid;

use iced::{
    button, pick_list, scrollable, slider, text_input, Alignment, Application, Button, Column,
    Command, Container, Element, Length, PickList, Row, Scrollable, Settings, Space, Subscription,
    Text, TextInput,
};

use image::ImageFormat;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use ajour_core::theme::ColorPalette;
use element::DEFAULT_PADDING;

static WINDOW_ICON: &[u8] = include_bytes!("../../resources/windows/ajour.ico");

static MAIN_MENU_VIEW: &str = "MAIN_MENU_VIEW";
static MAIN_SETTINGS_VIEW: &str = "MAIN_SETTINGS_VIEW";

pub struct Ajour {
    state: HashMap<Mode, State>,
    error: Option<anyhow::Error>,
    mode: Mode,
    config: Config,
    views: HashMap<Uuid, Box<dyn MessageHandlingView>>,
    view_labels: HashMap<&'static str, Uuid>,
    about_state: element::about::StateContainer,
    menu_state: element::menu::StateContainer,
    //settings_state: element::settings::StateContainer,
    //settings_view: Arc<RwLock<element::settings::View>>,
    scale_state: ScaleState,
    theme_state: ThemeState,
}

impl Default for Ajour {
    fn default() -> Self {
        let mut state = HashMap::new();
        state.insert(Mode::Catalog, State::Loading);
        Self {
            state,
            error: None,
            mode: Mode::Catalog,
            config: Config::default(),
            views: HashMap::new(),
            view_labels: HashMap::new(),
            about_state: Default::default(),
            menu_state: Default::default(),
            //settings_state: Default::default(),
            //settings_view: Arc::new(RwLock::new(Default::default())),
            scale_state: Default::default(),
            theme_state: Default::default(),
        }
    }
}

impl Ajour {
    fn create_view(&mut self, view_label: &'static str, view: Box<dyn MessageHandlingView>) {
        let uuid = Uuid::new_v4();
        self.view_labels.insert(view_label, uuid);
        self.views
            .insert(uuid, Box::new(element::settings::View::default()));
    }

    fn view_uuid_for_label(
        view_labels: &HashMap<&'static str, Uuid>,
        view_label: &'static str,
    ) -> Uuid {
        view_labels.get(view_label).unwrap().clone()
    }

    fn get_view<'a>(
        views: &'a mut HashMap<Uuid, Box<dyn MessageHandlingView>>,
        view_uuid: &Uuid,
    ) -> Option<&'a mut Box<dyn MessageHandlingView>> {
        views.get_mut(view_uuid)
    }

    fn create_views(&mut self) {
        self.create_view(
            MAIN_SETTINGS_VIEW,
            Box::new(element::settings::View::default()),
        );
        self.create_view(MAIN_MENU_VIEW, Box::new(element::menu::View::default()));
    }

}

pub trait MessageHandlingView {
    fn set_id(&mut self, new_id: &str);
    fn get_id(&self) -> &str;
    fn handle_message(&mut self, message: &Message) -> crate::Result<Command<Message>>;
    fn data_container<'a>(&'a mut self, color_palette: ColorPalette) -> Container<'a, Message>;
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    Error(anyhow::Error),
    Interaction(Interaction),
    RuntimeEvent(iced_native::Event),
    None(()),
}

impl Application for Ajour {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = Config;

    fn new(config: Config) -> (Self, Command<Message>) {
        let mut ajour = Ajour::default();
        ajour.create_views();
        /*ajour.views.insert(
            MAIN_SETTINGS_VIEW,
            Box::new(element::settings::View::default()),
        );*/
        (ajour, Command::batch(vec![]))
    }

    fn title(&self) -> String {
        String::from("Ajour")
    }

    fn scale_factor(&self) -> f64 {
        self.scale_state.scale
    }

    #[cfg(target_os = "windows")]
    fn should_exit(&self) -> bool {
        use crate::tray::SHOULD_EXIT;
        use std::sync::atomic::Ordering;

        SHOULD_EXIT.load(Ordering::Relaxed)
    }

    #[cfg(target_os = "windows")]
    fn mode(&self) -> iced::window::Mode {
        use crate::tray::GUI_VISIBLE;
        use iced::window::Mode;
        use std::sync::atomic::Ordering;

        if GUI_VISIBLE.load(Ordering::Relaxed) {
            Mode::Windowed
        } else {
            Mode::Hidden
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let runtime_subscription = iced_native::subscription::events().map(Message::RuntimeEvent);
        iced::Subscription::batch(vec![runtime_subscription])
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match update::handle_message(self, message) {
            Ok(x) => x,
            Err(e) => Command::perform(async { e }, Message::Error),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let view_labels = &self.view_labels;
        let color_palette = self
            .theme_state
            .themes
            .iter()
            .find(|(name, _)| name == &self.theme_state.current_theme_name)
            .as_ref()
            .unwrap_or(&&("Dark".to_string(), Theme::dark()))
            .1
            .palette;

        let mut content = Column::new();
        let view_uuid = Ajour::view_uuid_for_label(view_labels, MAIN_MENU_VIEW);

        {
            let views = &mut self.views;
            if let Some(menu_container) = Ajour::get_view(views, &view_uuid) {
                content = Column::new().push(menu_container.data_container(color_palette))
            }
        }

        // Spacer between menu and content.
        //content = content.push(Space::new(Length::Units(0), Length::Units(DEFAULT_PADDING)));

        match self.mode {
            Mode::About => {
                let about_container =
                    element::about::data_container(color_palette, &None, &mut self.about_state);
                content = content.push(about_container)
            }
            Mode::Settings => {
                let views = &mut self.views;
                let view_uuid = Ajour::view_uuid_for_label(view_labels, MAIN_SETTINGS_VIEW);
                if let Some(settings_container) = Ajour::get_view(views, &view_uuid) {
                    content = content.push(settings_container.data_container(color_palette))
                }
            }
            _ => {}
        }
        let container: Option<Container<Message>> = match self.mode {
            _ => None,
        };

        if let Some(c) = container {
            content = content.push(c);
        };

        // Finally wraps everything in a container.
        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::NormalBackgroundContainer(color_palette))
            .into()
    }
}

/// Starts the GUI.
/// This function does not return.
pub fn run(opts: Opts, config: Config) {
    // Set LANG using config (defaults to "en_US")
    LANG.set(RwLock::new(config.language.language_code()))
        .expect("setting LANG from config");

    log::debug!("config loaded:\n{:#?}", &config);

    let mut settings = Settings::default();
    settings.window.size = config.window_size.unwrap_or((900, 620));

    #[cfg(target_os = "windows")]
    {
        settings.exit_on_close_request = false;
    }

    #[cfg(not(target_os = "linux"))]
    // TODO (casperstorm): Due to an upstream bug, min_size causes the window to become unresizable
    // on Linux.
    // @see: https://github.com/ajour/ajour/issues/427
    {
        settings.window.min_size = Some((600, 300));
    }

    #[cfg(feature = "wgpu")]
    {
        let antialiasing = opts.antialiasing.unwrap_or(true);
        log::debug!("antialiasing: {}", antialiasing);
        settings.antialiasing = antialiasing;
    }

    #[cfg(feature = "opengl")]
    {
        let antialiasing = opts.antialiasing.unwrap_or(false);
        log::debug!("antialiasing: {}", antialiasing);
        settings.antialiasing = antialiasing;
    }

    // Sets the Window icon.
    let image = image::load_from_memory_with_format(WINDOW_ICON, ImageFormat::Ico)
        .expect("loading icon")
        .to_rgba8();
    let (width, height) = image.dimensions();
    let icon = iced::window::Icon::from_rgba(image.into_raw(), width, height);
    settings.window.icon = Some(icon.unwrap());

    settings.flags = config;

    // Runs the GUI.
    Ajour::run(settings).expect("running Ajour gui");
}

#[derive(Debug)]
pub enum State {
    Ready,
    Loading,
    Error(anyhow::Error),
}

impl Default for State {
    fn default() -> Self {
        State::Ready
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SelfUpdateStatus {
    InProgress,
    Failed,
}

impl std::fmt::Display for SelfUpdateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SelfUpdateStatus::InProgress => localized_string("updating"),
            SelfUpdateStatus::Failed => localized_string("failed"),
        };
        write!(f, "{}", s)
    }
}

#[derive(Default, Debug)]
pub struct SelfUpdateState {
    status: Option<SelfUpdateStatus>,
    btn_state: button::State,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    Catalog,
    Install,
    Settings,
    About,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Interaction {
    ViewInteraction(String),
    ModeSelected(Mode),
    ModeSelectedSettings(element::settings::Mode),
    //Expand(ExpandType),
    Ignore(String),
    SelectBackupDirectory(),
    OpenLink(String),
    Unignore(String),
    Update(String),
    ScaleUp,
    ScaleDown,
    Backup,
    ToggleHideIgnoredAddons(bool),
    CatalogQuery(String),
    InstallScmQuery(String),
    InstallScmUrl,
    UpdateAjour,
    AlternatingRowColorToggled(bool),
    KeybindingsToggle(bool),
    #[cfg(target_os = "windows")]
    ToggleCloseToTray(bool),
    #[cfg(target_os = "windows")]
    ToggleAutoStart(bool),
    #[cfg(target_os = "windows")]
    ToggleStartClosedToTray(bool),
}

pub struct ThemeState {
    themes: Vec<(String, Theme)>,
    current_theme_name: String,
    pick_list_state: pick_list::State<String>,
    input_state: text_input::State,
    input_url: String,
    import_button_state: button::State,
    open_builder_button_state: button::State,
}

impl Default for ThemeState {
    fn default() -> Self {
        let themes = Theme::all();

        ThemeState {
            themes,
            current_theme_name: "Dark".to_string(),
            pick_list_state: Default::default(),
            input_state: Default::default(),
            input_url: Default::default(),
            import_button_state: Default::default(),
            open_builder_button_state: Default::default(),
        }
    }
}

pub struct ScaleState {
    scale: f64,
    up_btn_state: button::State,
    down_btn_state: button::State,
}

impl Default for ScaleState {
    fn default() -> Self {
        ScaleState {
            scale: 1.0,
            up_btn_state: Default::default(),
            down_btn_state: Default::default(),
        }
    }
}