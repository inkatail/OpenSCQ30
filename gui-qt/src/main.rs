//! OpenSCQ30 Qt (QML) desktop UI entry point.
//!
//! The heavy lifting lives in `openscq30-lib`; this binary only boots a
//! `QGuiApplication` + `QQmlApplicationEngine` and exposes the
//! `SessionManager` QObject from `session.rs` to `qml/main.qml`.

pub mod model;
pub mod session;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// Use the KDE desktop style inside a Plasma session, Qt defaults elsewhere.
/// An explicit `QT_QUICK_CONTROLS_STYLE` always wins. When the style plugin
/// is missing, Qt warns and falls back to the default style.
fn configure_style() {
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_some() {
        return;
    }
    let in_plasma = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split([':', ';'])
        .any(|part| part.eq_ignore_ascii_case("kde"))
        || std::env::var_os("KDE_SESSION_VERSION").is_some();
    if in_plasma {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    // Stored preference, else system detection, else English fallback.
    if let Some(id) = model::effective_language(&model::config_path()) {
        if let Ok(language) = id.parse::<i18n_embed::unic_langid::LanguageIdentifier>() {
            openscq30_lib::i18n::init(std::slice::from_ref(&language));
        }
    }

    // Create the application and engine
    let mut app = QGuiApplication::new();
    // Wayland app-id / task-manager matching (see gui-qt/resources/*.desktop).
    QGuiApplication::set_desktop_file_name(&QString::from("com.oppzippy.OpenSCQ30.Qt"));
    configure_style();
    let mut engine = QQmlApplicationEngine::new();

    // Load the QML entry point into the engine
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/openscq30/guiqt/qml/main.qml"));
    }

    // Start the app
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
