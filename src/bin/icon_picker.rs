use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

const APP_ID: &str = "com.github.nwg_icon_picker";

struct AppState {
    icon_theme: gtk::IconTheme,
    icon_names: Vec<String>,
    result_wrapper_box: gtk::Box,
    result_scrolled_window: Option<gtk::ScrolledWindow>,
    icon_info: Option<IconInfo>,
    btn_height: i32,
    icon_path: String,
    gimp_available: bool,
    inkscape_available: bool,
}

impl AppState {
    fn new() -> Self {
        let icon_theme = gtk::IconTheme::default().expect("Failed to get icon theme");

        // Get all icon names
        let icon_names: Vec<String> = icon_theme
            .list_icons(None)
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            icon_theme,
            icon_names,
            result_wrapper_box: gtk::Box::new(gtk::Orientation::Vertical, 0),
            result_scrolled_window: None,
            icon_info: None,
            btn_height: 0,
            icon_path: String::new(),
            gimp_available: is_command("gimp"),
            inkscape_available: is_command("inkscape"),
        }
    }
}

fn is_command(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", cmd))
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("nwg-icon-picker")
        .default_width(600)
        .default_height(500)
        .build();

    let state = Rc::new(RefCell::new(AppState::new()));

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    main_box.set_margin_top(6);
    main_box.set_margin_bottom(6);
    main_box.set_margin_start(6);
    main_box.set_margin_end(6);

    // Search entry
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search icons..."));
    main_box.pack_start(&search_entry, false, false, 0);

    // Result wrapper box
    let result_wrapper_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    state.borrow_mut().result_wrapper_box = result_wrapper_box.clone();
    main_box.pack_start(&result_wrapper_box, true, true, 0);

    // Icon info panel
    let icon_info = IconInfo::new(
        "nwg-icon-picker",
        &state.borrow().icon_theme,
        state.borrow().gimp_available,
        state.borrow().inkscape_available,
    );
    state.borrow_mut().icon_info = Some(icon_info.clone());
    main_box.pack_start(&icon_info.widget, false, false, 6);

    // Connect search changed event
    let state_clone = Rc::clone(&state);
    search_entry.connect_search_changed(move |entry| {
        on_search_changed(entry, &state_clone);
    });

    window.add(&main_box);
    window.show_all();
}

fn on_search_changed(search_entry: &gtk::SearchEntry, state: &Rc<RefCell<AppState>>) {
    let phrase = search_entry.text().to_string();

    let mut state_mut = state.borrow_mut();

    if phrase.len() > 2 {
        // Remove old results
        if let Some(ref sw) = state_mut.result_scrolled_window {
            state_mut.result_wrapper_box.remove(sw);
        }

        // Create new scrolled window
        let scrolled_window =
            gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        scrolled_window.set_propagate_natural_width(true);
        scrolled_window.set_propagate_natural_height(true);

        let list_box = gtk::ListBox::new();
        scrolled_window.add(&list_box);

        // Add matching icons
        for name in &state_mut.icon_names {
            if name.contains(&phrase) {
                let row = create_icon_row(name, &state_mut.icon_theme, state);
                list_box.add(&row);
            }
        }

        state_mut
            .result_wrapper_box
            .pack_start(&scrolled_window, true, true, 0);
        state_mut.result_scrolled_window = Some(scrolled_window.clone());
        state_mut.result_wrapper_box.show_all();
    } else {
        // Clear results if phrase is too short
        if let Some(ref sw) = state_mut.result_scrolled_window {
            state_mut.result_wrapper_box.remove(sw);
            state_mut.result_scrolled_window = None;
        }
    }
}

fn create_icon_row(
    name: &str,
    icon_theme: &gtk::IconTheme,
    state: &Rc<RefCell<AppState>>,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let event_box = gtk::EventBox::new();
    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    // Load icon
    if let Ok(pixbuf) = icon_theme.load_icon(
        name,
        24,
        gtk::IconLookupFlags::FORCE_SIZE
            | gtk::IconLookupFlags::GENERIC_FALLBACK
            | gtk::IconLookupFlags::USE_BUILTIN,
    ) {
        if let Some(pixbuf) = pixbuf {
            let image = gtk::Image::from_pixbuf(Some(&pixbuf));
            hbox.pack_start(&image, false, false, 6);
        }
    }

    let label = gtk::Label::new(Some(name));
    hbox.pack_start(&label, false, false, 0);

    event_box.add(&hbox);
    row.add(&event_box);

    // Connect events
    let name_clone = name.to_string();
    let state_clone = Rc::clone(state);
    event_box.connect_button_press_event(move |_, _| {
        update_info(&name_clone, &state_clone);
        glib::Propagation::Proceed
    });

    let name_clone = name.to_string();
    let state_clone = Rc::clone(state);
    row.connect_focus_in_event(move |_, _| {
        update_info(&name_clone, &state_clone);
        glib::Propagation::Proceed
    });

    let name_clone = name.to_string();
    row.connect_activate(move |_| {
        on_row_activate(&name_clone);
    });

    row
}

fn update_info(name: &str, state: &Rc<RefCell<AppState>>) {
    let state_ref = state.borrow();
    if let Some(ref icon_info) = state_ref.icon_info {
        icon_info.update(name, &state_ref.icon_theme, state);
    }
}

fn on_row_activate(name: &str) {
    println!("{}", name);
    gtk::main_quit();
}

#[derive(Clone)]
struct IconInfo {
    widget: gtk::Box,
    button: gtk::Button,
    lbl_filename: gtk::Label,
    _btn_gimp: Option<gtk::Button>,
    _btn_inkscape: Option<gtk::Button>,
}

impl IconInfo {
    fn new(
        _name: &str,
        _icon_theme: &gtk::IconTheme,
        gimp_available: bool,
        inkscape_available: bool,
    ) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let button = gtk::Button::new();
        button.set_always_show_image(true);
        button.set_image_position(gtk::PositionType::Top);
        button.set_label("nwg-icon-picker");
        button.set_tooltip_text(Some("Click to pick the icon name"));
        widget.pack_start(&button, false, false, 0);

        button.connect_clicked(|btn| {
            on_button_clicked(btn);
        });

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        widget.pack_start(&hbox, false, false, 6);

        let lbl_filename = gtk::Label::new(None);
        lbl_filename.set_line_wrap(true);
        lbl_filename.set_selectable(true);
        hbox.pack_start(&lbl_filename, true, false, 0);

        let btn_gimp = if gimp_available {
            let btn = gtk::Button::from_icon_name(Some("gimp"), gtk::IconSize::Button);
            let btn_clone = btn.clone();
            btn.connect_clicked(move |_| on_btn_gimp(&btn_clone));
            hbox.pack_start(&btn, false, false, 0);
            Some(btn)
        } else {
            None
        };

        let btn_inkscape = if inkscape_available {
            let btn =
                gtk::Button::from_icon_name(Some("org.inkscape.Inkscape"), gtk::IconSize::Button);
            let btn_clone = btn.clone();
            btn.connect_clicked(move |_| on_btn_inkscape(&btn_clone));
            hbox.pack_start(&btn, false, false, 0);
            Some(btn)
        } else {
            None
        };

        Self {
            widget,
            button,
            lbl_filename,
            _btn_gimp: btn_gimp,
            _btn_inkscape: btn_inkscape,
        }
    }

    fn update(&self, name: &str, icon_theme: &gtk::IconTheme, state: &Rc<RefCell<AppState>>) {
        if let Some(info) = icon_theme.lookup_icon(name, 96, gtk::IconLookupFlags::empty()) {
            if let Some(filename) = info.filename() {
                let icon_path = filename.to_string_lossy().to_string();
                self.lbl_filename.set_text(&icon_path);
                state.borrow_mut().icon_path = icon_path;
            }
        }

        let image = gtk::Image::from_icon_name(Some(name), gtk::IconSize::Dialog);
        self.button.set_image(Some(&image));
        self.button.set_label(name);

        let btn_height = state.borrow().btn_height;
        if btn_height > 0 {
            self.button.set_size_request(-1, btn_height);
        }
    }
}

fn on_button_clicked(btn: &gtk::Button) {
    if let Some(label) = btn.label() {
        println!("{}", label);
    }
    gtk::main_quit();
}

fn on_btn_gimp(_btn: &gtk::Button) {
    // Implementation would open the icon in GIMP
    // You'd need access to the icon_path from state
    println!("Open in GIMP");
}

fn on_btn_inkscape(_btn: &gtk::Button) {
    // Implementation would open the icon in Inkscape
    // You'd need access to the icon_path from state
    println!("Open in Inkscape");
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run();
}
