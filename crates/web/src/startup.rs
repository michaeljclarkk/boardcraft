use std::{borrow::Cow, cell::RefCell, rc::Rc};

use gpui_kit::{App, AppContext, Application, ApplicationHandle, WindowOptions, component::Theme};
use wasm_bindgen::prelude::*;

use crate::shell::Shell;

thread_local! {
    static APPLICATION: RefCell<Option<ApplicationHandle>> = const { RefCell::new(None) };
}

// All icons used by the shell are embedded, including Kit's loading indicator.
gpui_kit::assets::icon_assets!(
    ShellAssets,
    [
        LoaderCircle,
        Check,
        Minus,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        Ellipsis,
        GripVertical,
        Search,
        PanelsTopLeft,
        Archive,
        Trash,
        ArrowRight
    ]
);

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    gpui_kit::platform::web_init();
    let platform = Rc::new(
        gpui_kit::web::WebPlatform::new_with_backend_and_font_fallback(
            false,
            gpui_kit::web::WebBackendPreference::Auto,
            gpui_kit::web::CanvasFontFallback::Disabled,
        ),
    );
    let app = Application::with_platform(platform).with_assets(ShellAssets);
    let launch = |cx: &mut App| {
        gpui_kit::init(cx);
        // The web backend's initial .SystemUIFont alias must resolve before Root
        // applies the theme. This font is embedded, never requested from a CDN.
        if cx
            .text_system()
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../fonts/IBMPlexSans-Regular.ttf"
            ))])
            .is_err()
        {
            web_sys::console::error_1(&"boardcraft: embedded font initialization failed".into());
            return;
        }
        Theme::update(cx, |theme| {
            theme.font_family = "IBM Plex Sans".into();
            theme.mono_font_family = "IBM Plex Sans".into();
        });
        if gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        })
        .is_err()
        {
            web_sys::console::error_1(&"boardcraft: shell window initialization failed".into());
        }
    };
    APPLICATION.with(|application| {
        *application.borrow_mut() = Some(app.run_embedded(launch));
    });
    Ok(())
}
