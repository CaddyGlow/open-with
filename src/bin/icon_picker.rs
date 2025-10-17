use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

const APP_ID: &str = "com.github.nwg_icon_picker";

struct AppState {
    icon_theme: gtk4::IconTheme,
    icon_names: Vec<String>,
    result_wrapper_box: gtk4::Box,
    result_scrolled_window: Option<gtk4::ScrolledWindow>,
    icon_info: Option<IconInfo>,
    btn_height: i32,
    icon_path: String,
    gimp_available: bool,
    inkscape_available: bool,
    window: gtk4::ApplicationWindow,
}

impl AppState {
    fn new(window: &gtk4::ApplicationWindow) -> Self {
        let icon_theme = gtk4::IconTheme::for_display(&gtk4::prelude::WidgetExt::display(window));

        // Get all icon names
        let icon_names: Vec<String> = icon_theme
            .icon_names()
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            icon_theme,
            icon_names,
            result_wrapper_box: gtk4::Box::new(gtk4::Orientation::Vertical, 0),
            result_scrolled_window: None,
            icon_info: None,
            btn_height: 0,
            icon_path: String::new(),
            gimp_available: is_command("gimp"),
            inkscape_available: is_command("inkscape"),
            window: window.clone(),
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

    let state = Rc::new(RefCell::new(AppState::new(&window)));

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    main_box.set_margin_top(6);
    main_box.set_margin_bottom(6);
    main_box.set_margin_start(6);
    main_box.set_margin_end(6);

    // Search entry
    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search icons..."));
    main_box.append(&search_entry);

    // Result wrapper box
    let result_wrapper_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    state.borrow_mut().result_wrapper_box = result_wrapper_box.clone();
    main_box.append(&result_wrapper_box);

    // Icon info panel
    let icon_info = IconInfo::new(
        "nwg-icon-picker",
        &state.borrow().icon_theme,
        state.borrow().gimp_available,
        state.borrow().inkscape_available,
        &state,
    );
    state.borrow_mut().icon_info = Some(icon_info.clone());
    main_box.append(&icon_info.widget);

    // Connect search changed event
    let state_clone = Rc::clone(&state);
    search_entry.connect_search_changed(move |entry| {
        on_search_changed(entry, &state_clone);
    });

    window.set_child(Some(&main_box));
    window.present();
}

fn on_search_changed(search_entry: &gtk4::SearchEntry, state: &Rc<RefCell<AppState>>) {
    let phrase = search_entry.text().to_string();

    let mut state_mut = state.borrow_mut();

    if phrase.len() > 2 {
        // Remove old results
        if let Some(ref sw) = state_mut.result_scrolled_window {
            state_mut.result_wrapper_box.remove(sw);
        }

        // Create new scrolled window
        let scrolled_window = gtk4::ScrolledWindow::new();
        scrolled_window.set_propagate_natural_width(true);
        scrolled_window.set_propagate_natural_height(true);

        let list_box = gtk4::ListBox::new();
        scrolled_window.set_child(Some(&list_box));

        // Add matching icons
        for name in &state_mut.icon_names {
            if name.contains(&phrase) {
                let row = create_icon_row(name, &state_mut.icon_theme, state);
                list_box.append(&row);
            }
        }

        state_mut.result_wrapper_box.append(&scrolled_window);
        state_mut.result_scrolled_window = Some(scrolled_window.clone());
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
    icon_theme: &gtk4::IconTheme,
    state: &Rc<RefCell<AppState>>,
) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);

    // Load icon using the new GTK4 API
    let paintable = icon_theme.lookup_icon(
        name,
        &[],
        24,
        1,
        gtk4::TextDirection::Ltr,
        gtk4::IconLookupFlags::empty(),
    );
    let image = gtk4::Image::from_paintable(Some(&paintable));
    hbox.append(&image);

    let label = gtk4::Label::new(Some(name));
    label.set_margin_start(6);
    hbox.append(&label);

    row.set_child(Some(&hbox));

    // Create gesture for button press
    let gesture = gtk4::GestureClick::new();
    gesture.set_button(gtk4::gdk::ffi::GDK_BUTTON_PRIMARY as u32);
    let name_clone = name.to_string();
    let state_clone = Rc::clone(state);
    gesture.connect_pressed(move |_, _, _, _| {
        update_info(&name_clone, &state_clone);
    });
    row.add_controller(gesture);

    // Connect focus event - use idle_add to avoid borrow conflicts during focus traversal
    let name_clone = name.to_string();
    let state_clone = Rc::clone(state);
    let event_controller = gtk4::EventControllerFocus::new();
    event_controller.connect_enter(move |_| {
        let name = name_clone.clone();
        let state = state_clone.clone();
        glib::idle_add_local_once(move || {
            update_info(&name, &state);
        });
    });
    row.add_controller(event_controller);

    // Connect activate signal
    let name_clone = name.to_string();
    let state_clone = Rc::clone(state);
    row.connect_activate(move |_| {
        on_row_activate(&name_clone, &state_clone);
    });

    row
}

fn update_info(name: &str, state: &Rc<RefCell<AppState>>) {
    let (icon_info_opt, icon_theme) = {
        let state_ref = state.borrow();
        (state_ref.icon_info.clone(), state_ref.icon_theme.clone())
    };

    if let Some(icon_info) = icon_info_opt {
        icon_info.update(name, &icon_theme, state);
    }
}

fn on_row_activate(name: &str, state: &Rc<RefCell<AppState>>) {
    println!("{}", name);
    state.borrow().window.close();
}

#[derive(Clone)]
struct IconInfo {
    widget: gtk4::Box,
    button: gtk4::Button,
    button_image: gtk4::Image,
    lbl_filename: gtk4::Label,
    _btn_gimp: Option<gtk4::Button>,
    _btn_inkscape: Option<gtk4::Button>,
}

impl IconInfo {
    fn new(
        _name: &str,
        _icon_theme: &gtk4::IconTheme,
        gimp_available: bool,
        inkscape_available: bool,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.set_margin_top(6);

        let button = gtk4::Button::new();
        let button_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);

        let button_image = gtk4::Image::from_icon_name("nwg-icon-picker");
        button_image.set_pixel_size(48);
        button_box.append(&button_image);

        let button_label = gtk4::Label::new(Some("nwg-icon-picker"));
        button_box.append(&button_label);

        button.set_child(Some(&button_box));
        button.set_tooltip_text(Some("Click to pick the icon name"));
        widget.append(&button);

        let state_clone = Rc::clone(state);
        button.connect_clicked(move |btn| {
            on_button_clicked(btn, &state_clone);
        });

        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        hbox.set_margin_top(6);
        widget.append(&hbox);

        let lbl_filename = gtk4::Label::new(None);
        lbl_filename.set_wrap(true);
        lbl_filename.set_selectable(true);
        lbl_filename.set_hexpand(true);
        hbox.append(&lbl_filename);

        let btn_gimp = if gimp_available {
            let btn = gtk4::Button::new();
            let img = gtk4::Image::from_icon_name("gimp");
            img.set_pixel_size(16);
            btn.set_child(Some(&img));
            let state_clone = Rc::clone(state);
            btn.connect_clicked(move |_| on_btn_gimp(&state_clone));
            hbox.append(&btn);
            Some(btn)
        } else {
            None
        };

        let btn_inkscape = if inkscape_available {
            let btn = gtk4::Button::new();
            let img = gtk4::Image::from_icon_name("org.inkscape.Inkscape");
            img.set_pixel_size(16);
            btn.set_child(Some(&img));
            let state_clone = Rc::clone(state);
            btn.connect_clicked(move |_| on_btn_inkscape(&state_clone));
            hbox.append(&btn);
            Some(btn)
        } else {
            None
        };

        Self {
            widget,
            button,
            button_image,
            lbl_filename,
            _btn_gimp: btn_gimp,
            _btn_inkscape: btn_inkscape,
        }
    }

    fn update(&self, name: &str, icon_theme: &gtk4::IconTheme, state: &Rc<RefCell<AppState>>) {
        // Look up icon file path
        let paintable = icon_theme.lookup_icon(
            name,
            &[],
            96,
            1,
            gtk4::TextDirection::Ltr,
            gtk4::IconLookupFlags::empty(),
        );

        let btn_height = {
            let mut state_mut = state.borrow_mut();
            if let Some(file) = paintable.file() {
                if let Some(path) = file.path() {
                    let icon_path = path.to_string_lossy().to_string();
                    self.lbl_filename.set_text(&icon_path);
                    state_mut.icon_path = icon_path;
                }
            }
            state_mut.btn_height
        };

        // Update button image
        self.button_image.set_icon_name(Some(name));
        self.button_image.set_pixel_size(48);

        // Update button label by accessing the box child
        if let Some(button_box) = self.button.child().and_downcast::<gtk4::Box>() {
            if let Some(label) = button_box.last_child().and_downcast::<gtk4::Label>() {
                label.set_text(name);
            }
        }

        if btn_height > 0 {
            self.button.set_size_request(-1, btn_height);
        }
    }
}

fn on_button_clicked(btn: &gtk4::Button, state: &Rc<RefCell<AppState>>) {
    // Get the label from the button's box child
    if let Some(button_box) = btn.child().and_downcast::<gtk4::Box>() {
        if let Some(label) = button_box.last_child().and_downcast::<gtk4::Label>() {
            println!("{}", label.text());
        }
    }
    state.borrow().window.close();
}

fn on_btn_gimp(state: &Rc<RefCell<AppState>>) {
    let icon_path = state.borrow().icon_path.clone();
    if !icon_path.is_empty() {
        let _ = Command::new("gimp").arg(&icon_path).spawn();
    }
}

fn on_btn_inkscape(state: &Rc<RefCell<AppState>>) {
    let icon_path = state.borrow().icon_path.clone();
    if !icon_path.is_empty() {
        let _ = Command::new("inkscape").arg(&icon_path).spawn();
    }
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run();
}
