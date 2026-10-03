use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new()
        .qml_module(QmlModule {
            uri: "com.openscq30.guiqt",
            rust_files: &["src/session.rs"],
            qml_files: &["qml/main.qml"],
            ..Default::default()
        })
        .build();
}
