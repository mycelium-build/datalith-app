use android_activity::AndroidApp;
use gpui_mobile::android::jni as platform;
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::jstring,
};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

static SELECTED: Mutex<Option<PathBuf>> = Mutex::new(None);

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    platform::install_panic_hook();
    let _platform = platform::init_platform(&app);
    if let (Some(platform), Some(files)) = (platform::shared_platform(), app.internal_data_path()) {
        datalith_mobile::run_android(
            gpui_kit::Application::with_platform(platform.into_rc()),
            &files,
            SELECTED
                .lock()
                .ok()
                .and_then(|mut selected| selected.take()),
        );
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_VaultActivity_prepareVault(
    mut env: JNIEnv,
    _: JClass,
    documents: JString,
) -> jstring {
    let Some(path) = env.get_string(&documents).ok().map(String::from) else {
        return std::ptr::null_mut();
    };
    datalith_mobile::starter(Path::new(&path))
        .ok()
        .and_then(|root| env.new_string(root.to_string_lossy()).ok())
        .map_or(std::ptr::null_mut(), JString::into_raw)
}

#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_DocumentActivity_selectDocument(
    mut env: JNIEnv,
    _: JClass,
    path: JString,
) {
    if let Ok(path) = env.get_string(&path)
        && let Ok(mut selected) = SELECTED.lock()
    {
        *selected = Some(PathBuf::from(String::from(path)));
    }
}
